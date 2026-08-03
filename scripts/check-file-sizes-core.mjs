import { execFileSync } from "node:child_process";
import { promises as fs } from "node:fs";
import path from "node:path";

function git(args, cwd, options = {}) {
  return execFileSync("git", args, {
    cwd,
    encoding: "utf8",
    maxBuffer: 10 * 1024 * 1024,
    ...options,
  });
}

function toPosixPath(relativePath) {
  return relativePath.split(path.sep).join("/");
}

export function countLines(content) {
  if (content.length === 0) {
    return 0;
  }
  return content.split(/\r?\n/).length;
}

export function allowedLineCount(baseLines, maxLines) {
  return baseLines == null || baseLines <= maxLines ? maxLines : baseLines;
}

export function evaluateFileSize({ baseLines, candidateLines, maxLines }) {
  const limit = allowedLineCount(baseLines, maxLines);
  return { limit, violates: candidateLines > limit };
}

function commitParents(repoRoot, ref) {
  return git(["show", "-s", "--format=%P", ref], repoRoot)
    .trim()
    .split(/\s+/)
    .filter(Boolean);
}

function validatedUpstreamSyncBase(repoRoot, ref) {
  const parents = commitParents(repoRoot, ref);
  if (parents.length !== 2) return null;

  const message = git(["show", "-s", "--format=%B", ref], repoRoot);
  const trailers = [
    ...message.matchAll(/^Upstream-Sync-Base:\s*([0-9a-f]{40})\s*$/gim),
  ];
  if (trailers.length !== 1) return null;

  const declaredBase = trailers[0][1].toLowerCase();
  return declaredBase === parents[1].toLowerCase() ? parents[1] : null;
}

function resolveCiUpstreamSyncBase(repoRoot) {
  const directBase = validatedUpstreamSyncBase(repoRoot, "HEAD");
  if (directBase) return directBase;

  const headParents = commitParents(repoRoot, "HEAD");
  if (headParents.length !== 2) return null;

  const branchOnlyCommits = git(
    ["rev-list", "--first-parent", headParents[1], `^${headParents[0]}`],
    repoRoot,
  )
    .trim()
    .split("\n")
    .filter(Boolean);

  for (const commit of branchOnlyCommits) {
    const upstreamBase = validatedUpstreamSyncBase(repoRoot, commit);
    if (upstreamBase) return upstreamBase;
  }
  return null;
}

function findRule(rules, relativePath) {
  return rules.find((rule) => relativePath.startsWith(`${rule.root}/`));
}

export function resolveBaseRef(repoRoot, env = process.env) {
  if (env.CHECK_FILE_SIZES_BASE) {
    return env.CHECK_FILE_SIZES_BASE;
  }

  if (env.GITHUB_ACTIONS === "true") {
    return resolveCiUpstreamSyncBase(repoRoot) ?? "HEAD^1";
  }

  try {
    const mergeBase = git(
      ["merge-base", "origin/main", "HEAD"],
      repoRoot,
    ).trim();
    const head = git(["rev-parse", "HEAD"], repoRoot).trim();
    return mergeBase === head ? "HEAD" : mergeBase;
  } catch (error) {
    throw new Error(
      "Could not resolve the file-size base from origin/main. Fetch origin/main or set CHECK_FILE_SIZES_BASE to an explicit commit.",
      { cause: error },
    );
  }
}

export function parseChangedFiles(output) {
  const fields = output.split("\0");
  const changes = [];

  for (let index = 0; index < fields.length - 1; ) {
    const status = fields[index++];
    if (status.startsWith("R") || status.startsWith("C")) {
      changes.push({
        status: status[0],
        oldPath: fields[index++],
        path: fields[index++],
      });
    } else {
      changes.push({ status: status[0], path: fields[index++] });
    }
  }

  return changes;
}

function changedProjectFiles({ repoRoot, projectRelative, baseRef }) {
  const output = git(
    ["diff", "--name-status", "-z", "-M", baseRef, "--", projectRelative],
    repoRoot,
  );
  const changes = parseChangedFiles(output);
  const trackedPaths = new Set(changes.map((change) => change.path));
  const untracked = git(
    ["ls-files", "--others", "--exclude-standard", "-z", "--", projectRelative],
    repoRoot,
  )
    .split("\0")
    .filter(Boolean);

  for (const filePath of untracked) {
    if (!trackedPaths.has(filePath)) {
      changes.push({ status: "A", path: filePath });
    }
  }
  return changes;
}

function readBaseFile(repoRoot, baseRef, filePath) {
  return git(["show", `${baseRef}:${filePath}`], repoRoot, {
    encoding: null,
  }).toString("utf8");
}

export async function runFileSizeCheck({ projectRoot, rules, label }) {
  // Every governed project is a direct child of the repository root. Derive
  // these paths without Git so hook-provided repository environment variables
  // cannot collapse the project pathspec to an empty string.
  const repoRoot = path.dirname(projectRoot);
  const projectRelative = toPosixPath(path.basename(projectRoot));
  const baseRef = resolveBaseRef(repoRoot);

  // Fail clearly instead of silently turning a missing/shallow base into a pass.
  git(["cat-file", "-e", `${baseRef}^{commit}`], repoRoot);

  const violations = [];
  for (const change of changedProjectFiles({
    repoRoot,
    projectRelative,
    baseRef,
  })) {
    if (change.status === "D") continue;

    const relativePath = toPosixPath(
      path.relative(projectRelative, change.path),
    );
    const rule = findRule(rules, relativePath);
    if (!rule || !rule.extensions.has(path.extname(relativePath))) continue;

    const candidatePath = path.join(repoRoot, change.path);
    const candidateLines = countLines(await fs.readFile(candidatePath, "utf8"));
    const basePath = change.oldPath ?? change.path;
    const baseContent =
      change.status === "A" ? null : readBaseFile(repoRoot, baseRef, basePath);
    const baseLines = baseContent == null ? null : countLines(baseContent);
    const result = evaluateFileSize({
      baseLines,
      candidateLines,
      maxLines: rule.maxLines,
    });

    if (result.violates) {
      violations.push({
        relativePath,
        baseLines,
        candidateLines,
        limit: result.limit,
      });
    }
  }

  if (violations.length === 0) return;

  console.error(`${label} file size ratchet failed (base ${baseRef}):`);
  for (const violation of violations) {
    const before = violation.baseLines == null ? "new" : violation.baseLines;
    const delta =
      violation.baseLines == null
        ? ""
        : ` (${violation.candidateLines - violation.baseLines >= 0 ? "+" : ""}${violation.candidateLines - violation.baseLines})`;
    console.error(
      `- ${violation.relativePath}: ${before} -> ${violation.candidateLines}${delta} lines (allowed ${violation.limit})`,
    );
  }
  console.error(
    "Keep new files at or below the limit; files already over it may not grow.",
  );
  process.exitCode = 1;
}
