---
title: Introduction
description: What Ought is and why it exists.
order: 1
---

Ought is a testing tool for software systems. You write what your system **ought** to do as plain markdown. An LLM reads your spec, reads your source code, and generates the tests. You run them with the `ought` CLI.

## The problem

Test intent and test implementation are fused together in code. The assertion `assert_eq!(response.status(), 401)` buries the intent — _"invalid credentials must return 401"_ — inside mechanical setup and plumbing. When requirements change, you rewrite test code instead of updating a sentence.

Ought pulls intent up into a human-readable spec and delegates the mechanical work to an LLM.

## How it differs from other test tools

Traditional test frameworks give you a way to write assertions about source code. Ought adds a layer above that: a spec that says, in plain language, what the assertions are _for_. Every test is traceable back to a clause in a spec, and every clause in a spec is traceable forward to the tests that enforce it.

## Where to go next

- [Installation](/products/ought/docs/installation) — get the CLI on your machine
- [Quick start](/products/ought/docs/quickstart) — write your first spec and run it
- [Writing specs](/products/ought/docs/writing-specs) — the spec file format in detail
