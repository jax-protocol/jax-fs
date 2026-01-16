---
description: Load a Linear issue and plan the implementation
argument-hint: <issue-id>
allowed-tools:
  - mcp__linear-gen__get_issue
  - mcp__linear-gen__list_comments
  - mcp__linear-gen__create_comment
  - Read
  - Glob
  - Grep
---

Load a Linear issue, analyze requirements, and create an implementation plan.

## Steps

1. If no issue ID provided in `$ARGUMENTS`, ask the user for the issue ID.

2. Fetch issue details using `mcp__linear-gen__get_issue`:
   - Get title, description, acceptance criteria
   - Note the current state and assignee

3. Get existing comments using `mcp__linear-gen__list_comments`:
   - Look for additional context, clarifications, or requirements

4. Post a comment on the issue indicating work has started:
   ```
   Starting work on this issue.
   ```

5. Analyze the requirements:
   - What needs to be built?
   - What are the acceptance criteria?
   - Are there any ambiguities or missing details?

6. Explore the relevant codebase:
   - Search for related files and patterns
   - Understand existing implementations that might be affected
   - Identify files that will need to be modified

7. Enter planning mode using `EnterPlanMode`:
   - Create a detailed implementation plan
   - Include specific files to modify
   - Include verification steps

If there are unclear requirements, ask the user for clarification before finalizing the plan.
