# GitHub Branch and Release Governance Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Preserve Sipher modifications while keeping upstream synchronization deliberate and owner-controlled.

**Architecture:** `develop` is the default integration branch. Feature branches merge into `develop`; release PRs promote tested changes from `develop` to `main`. Upstream synchronization happens only when the repository owner creates a dedicated sync branch and PRs it into `main`; there is no scheduled sync job.

**Tech Stack:** Git, GitHub branches, pull requests, branch protection, CI.

---

### Task 1: Establish branch roles

- Keep `origin` pointed at `sipherxyz/buzz` and `upstream` pointed at `block/buzz`.
- Create and push `develop`, then set it as the GitHub default branch.
- Document feature, release, hotfix, and owner-managed upstream-sync flows.

### Task 2: Protect integration and release branches

- Require pull requests and passing CI on `develop`.
- Require pull requests, passing CI, and no force pushes on `main`.
- Allow repository owners to create dedicated upstream-sync branches only when needed.

### Task 3: Validate the release path

- Merge feature work into `develop`.
- Open a release PR from `develop` to `main`.
- Deploy only the merge commit present on `main` and record that SHA.
