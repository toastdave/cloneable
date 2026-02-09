#!/bin/bash

opencode --model openai/gpt-5.2-codex --agent build --prompt "@PRD.jsonc @progress.txt \
  1. Find the highest-priority task and implement it. \
    - Utilize these skills when needed for your task: \ 
        - frontend-design skill for any design decisions \
        - shadcn-ui skill for TailwindCSS and ShadCN UI implementation \
        - tauri-v2 skill for Tauri implementation \
        - rust-best-practices skill for Rust implementation \
  2. Run your tests and type checks. \
  3. Update the PRD with what was done. \
  4. After completing each task, append to progress.txt:
- Task completed and PRD item reference
- Key decisions made and reasoning
- Files changed
- Any blockers or notes for next iteration
Keep entries concise. Sacrifice grammar for the sake of concision. This file helps future iterations skip exploration.\
  5. Commit your changes using conventional commits. \
  ONLY WORK ON A SINGLE TASK."