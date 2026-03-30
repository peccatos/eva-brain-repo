# for OpenAI

## What this repository is

`eva-brain-repo` is a standalone demo repository for EVA.
It is intentionally not a copy of the original monolith.
The goal is to show a compact, runnable slice of the system with a clear CLI surface and visible outputs.

Repository: https://github.com/peccatos/eva-brain-repo

## What is included

This demo keeps only the parts needed for a believable end-to-end local workflow:

- `cargo run`
  - prints a structured Russian phase report for the current runtime state
- `cargo run -- --repo <REPO_URL>`
  - clones or copies a repository
  - analyzes it
  - applies a small but real patch plan
  - writes `eva_output/report.md` and `eva_output/summary.json`
- `cargo run --bin github_repo_discover`
  - repository discovery layer
- `cargo run --bin github_repo_prepare`
  - local clone/prepare/reproduce stage
- `cargo run --bin benchmark_batch`
  - benchmark execution over prepared cases

## What is already proven

The following paths were run locally and passed:

- `cargo check`
- `cargo test`
- `cargo run`
- `cargo run -- --repo <local_fixture_repo>`
- `cargo run --bin github_repo_discover -- --fixture fixtures/github_search_fixture.json`
- `cargo run --bin github_repo_prepare -- fixtures/benchmark_discovery_manifest.json fixtures/benchmark_prepared.json fixtures/benchmark_ready.json`
- `cargo run --bin benchmark_batch -- fixtures/benchmark_ready.json fixtures/benchmark_batch_report.json 1`

What that proves:

- the repository is buildable
- the main CLI path works
- the repo patch report contract works
- the offline benchmark smoke path works
- the demo can show repair activation signals, mutation attempts, and rollback metrics

## What is not proven

This repository should not be described as a stable autonomous repair engine.
That would be false.

Not proven yet:

- stable successful repair on a meaningful batch of real open-source repositories
- strong semantic repair for complex Rust projects or Cargo workspace graphs
- background autonomous execution
- large-scale parallel repair

Current honest position:

- reproduction path: present
- reporting path: present
- bounded mutation path: present
- benchmark smoke path: present
- robust real-world repair quality: not proven

## Why this repo exists

The main reason is demonstration quality.
A compact repo is easier to inspect, run, share, and discuss than a large evolving monolith with experimental subsystems.

This repo is meant to answer a simple question quickly:

> What does EVA actually do today, and what can be run right now?

## Recommended way to evaluate it

1. Run `cargo run`
2. Run `cargo run -- --repo <REPO_URL>` on a small Rust repo
3. Run the benchmark demo binaries on local fixtures
4. Inspect generated reports rather than relying on claims

## Important limitation

The current benchmark path can show mutation attempts and rollback, but that is not the same thing as reliable code repair.
The distinction matters.

## Short summary

This repository is a clean demo baseline for EVA.
It is good enough to run, inspect, and extend.
It is not yet evidence of production-grade autonomous repair.
