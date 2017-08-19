Tailor
======

Checks
------

### Max Body Line Length

Validates that the line length in the body of the commit do not exceed a given length.

#### Name

`max_body_line_length`

### Max Summary Length

Validates that the line length of the commit summary does not exceed a given length.

#### Name

`max_summary_length`

### No Capitalize

Validates that there are no capital letters present in the commit summary.

#### Name

`no_capitalize_summary`

### No Fixup

Validates that the commit summary does not start with `fixup!`.

#### Name

`no_fixup`

### No Squash

Validates that the commit summary does not start with `squash!`.

#### Name

`no_squash`

### No WIP

Validates that the commit summary does not start with `wip`.

#### Name

`no_wip`

### Requires Body

Validates that the commit data contains a body.

#### Name

`requires_body`

### Summary Scope

Validates that the commit summary contains a scope. [Click here for more information.](https://github.com/coreos/ignition/blob/master/CONTRIBUTING.md#format-of-the-commit-message)

#### Name

`summary_scope`

Tailor Disable
--------------

Administrators of repos using tailor can write a comment to disable individual tailor checks (or all tailor checks) via the following syntax: `tailor disable <check_name>` (to disable all tailor checks use `tailor disable all`). To re-enable the checks delete the comment and tailor will re-check the repo.
