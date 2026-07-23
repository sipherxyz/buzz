# Mac Studio Source Deployment Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Deploy the Buzz relay from the merged `main` branch on the company Mac Studio with safe preflight, local commit-tagged images, backups, health checks, and rollback.

**Architecture:** An operator SSHes to the Mac Studio and runs a versioned script from the repository. The script requires a clean `main`, fetches and fast-forwards from `origin/main`, validates host capacity and production configuration, builds `sipher-buzz:<short-sha>` locally, starts the production Compose stack against external S3, verifies health, and records deployment state. Existing commit-tagged images provide rollback without rebuilding.

**Tech Stack:** POSIX shell, Git, Docker/OrbStack, Docker Compose, PostgreSQL, Redis, external S3-compatible storage.

---

### Task 1: Specify deployment inputs and gates

- Add shell tests for the pure resource and external-storage checks; exercise branch, clean-tree, Docker, secrets, FileVault, and firewall checks during the remote preflight.
- Add `deploy/macstudio/.env.example` containing non-secret production inputs and placeholders.
- Implement a read-only `preflight.sh` that exits non-zero with actionable messages.

### Task 2: Add the source deployment workflow

- Add shell tests for fast-forward-only update, commit-derived image tags, build-before-cutover, deployment state recording, and failure propagation.
- Implement `deploy.sh` with dry-run support and explicit repository/Compose paths.
- Build the image from the checked-out commit, then start services without bundling MinIO.

### Task 3: Add backup, health verification, and rollback

- Add tests for PostgreSQL backup naming, previous-image preservation, and the migration-change rollback guard.
- Implement backup and rollback scripts that never delete the current database or repository.
- Document recovery steps and the exact state files/operators must preserve.

### Task 4: Validate on the Mac Studio

- Copy or clone the repository into an explicit application directory.
- Run preflight remotely and stop if any production gate fails.
- After disk, memory, storage, network, and host-security gates pass, deploy the merged `main`, verify health from the LAN, and record the deployed commit.
