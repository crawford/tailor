# Tailor #

Tailor pull requests to your liking.

Tailor is a GitHub bot which can be used to validate that pull requests comply with a set of rules. It can be used for example to ensure that commit messages are properly formatted, that commits are properly structured, or that pull requests have particular labels.

## Usage ##

### Repository Configuration ###

Each repository contains its own rules and configuration. This makes it easy to version the configuration alongside the code. The configuration is stored in `/.github/tailor.yaml` and has the following structure:

```yaml
# This is the list of rules that are applied to each of the pull requests to
# the repository.
rules:

    # The name is used as an identifier and can be used in the admin commands.
  - name:        commit title

    # The description is printed in the status messages to inform the submitter
    # of what is wrong with their pull request.
    description: all commit titles are less than or equal to 50 characters

    # The expression describes the rule itself. Refer to the language
    # documentation for an overview of the available functionality.
    expression:  .commits all(.message test "^.{0,50}$")
```

Each of the rules are run on the entire pull request (the [root context](README.md#root-context)). They are run independently and cannot influence one another. Often times, it is useful to use `.commits all` to run an expression on each of the commits in the pull request, requiring all of them to comply. This is detailed further in the [Expressions section](README.md#expressions). The rule expression must result in a boolean value, `true` indicating a success and `false` a failure.

#### Expressions ####

Expressions are made up of a series of infix operators detailed below. Almost all operators have an input value (on the left) and most have an argument value (on the right). Expressions are evaluated from left to right, which the result of the previous operator feeding into the next. Parenthesis can be used to evaluate a contained expression before its surroundings.

The only operator which doesn't take an input is the context operator (`.`). The result of this operation is the current context. It can be further limited by using a specifier (e.g. `.commits`) if the context is a dictionary. Since the context operator is the only operator which doesn't take an input, all expressions must begin with it.

It might be helpful to break down a few examples.

This expression returns `true` if there are exactly ten commits in the pull request: `.commits length = 10`. Parenthesis around every operation could be added for clarity: `(((.commits) length) = 10)`

This expression returns `true` if every commit message is no more than fifty characters: `.commits all(.message test "^.{0,50}$")`. This expression makes use of the `all` operator, which is used for manipulating lists. For every commit, the expression `.message test "^.{0,50}$"` is evaluated with the context set to the commit in question. This inner expression then uses a context specifier (`.message`) to get the commit message and uses `test` to see if there are more than fifty characters. If every one of the inner expressions evaluates to `true`, `all` also results in `true`.

There are a handful of other operators, detailed below.

##### Operators #####

###### Comparison ######

| Operator |   Input   |  Argument  |  Result |                           Description                           |
|:--------:|:---------:|:----------:|:-------:|:----------------------------------------------------------------|
|    `=`   | Any value |  Any value | Boolean | `true` if the values are equal                                  |
|    `<`   |  Numeral  |   Numeral  | Boolean | `true` if the input is less than the argument                   |
|    `>`   |  Numeral  |   Numeral  | Boolean | `true` if the input is greater than the argument                |

###### Logical ######

| Operator |   Input   |  Argument  |  Result |                           Description                           |
|:--------:|:---------:|:----------:|:-------:|:----------------------------------------------------------------|
|   `and`  |  Boolean  |   Boolean  | Boolean | `true` if both the input and argument are `true`                |
|    `or`  |  Boolean  |   Boolean  | Boolean | `true` if either the input or argument are `true`               |
|   `xor`  |  Boolean  |   Boolean  | Boolean | `true` if one of the input or argument are `true`               |
|   `not`  |  Boolean  |            | Boolean | `true` if the input is `false`                                  |

###### List Manipulation ######

Each of the list manipulation operators (except `length`), accepts an expression as an argument and this expression is evaluated for each of the elements in the list. The context for each expression is set to the corresponding element from the input list. As mentioned above, every expression must begin with a context operator.

| Operator |   Input   |  Argument  |  Result |                           Description                           |
|:--------:|:---------:|:----------:|:-------:|:----------------------------------------------------------------|
|   `all`  |    List   | Expression | Boolean | `true` if all of the expression results are `true`              |
|   `any`  |    List   | Expression | Boolean | `true` if any of the expression results are `true`              |
| `filter` |    List   | Expression |   List  | List of each of the elements who's expression results in `true` |
|   `map`  |    List   | Expression |   List  | List the result of each elements expression                     |
| `length` |    List   |            | Numeral | The number of elements in the list                              |

###### Miscellaneous ######

| Operator |   Input   |  Argument  |  Result |                           Description                           |
|:--------:|:---------:|:----------:|:-------:|:----------------------------------------------------------------|
|  `test`  |   String  |   String   | Boolean | `true` if the argument (a regular expression) matches the input |
|    `.`   |           |            |  Value  | The current context                                             |

##### Values #####

There are a few different types of values that can be used in Tailor:

  - numeral - Any positive number (e.g. `25`)
  - boolean - `true` or `false`
  - string - A sequence of characters delimited by double-quotes (e.g. `"this is a string"`). Double-quotes and backslashes can be escaped with a backslash so they can be included in the string (e.g. `"Escaped \"quotes\""`)
  - list - A sequence of values delimited by brackets (e.g. `[1 2 3]`)
  - dictionary - A mapping between strings (the key) and values (the value). These cannot be created in expressions so they are only useful for context specifiers.

#### Root Context ####

The root context is the initial input (a dictionary) into the rule expression. It is always a dictionary of values, derived from the pull request, and is of the following structure. Dictionaries are denoted by indentation, lists are denoted with brackets, and all leaf members are strings.

```
.
  .user
    .login
  .title
  .body
  .commits[]
    .sha
    .author
      .name
      .email
      .date
      .github_login
    .committer
      .name
      .email
      .date
      .github_login
    .message
  .comments[]
    .user
      .login
    .body
    .created_at
```

### Admin Commands ###

In some cases, it may be necessary to grant an exemption to the rules. Repository admins can specify exemptions by commenting on the pull request with `tailor disable <rule name>` to disable a particular rule or `tailor disable all` to disable all rules. Exemptions can be removed by deleting the comment.

## Setup ##

### Building ###

After cloning the repository, Tailor can be built and run with `cargo run`. The logging verbosity can be increased by adding up to three `-v` flags to the invocation (`cargo run -- -vvv`).

### Configuring GitHub ###

Tailor is designed to be used as a webhook. Each GitHub repository will need to be configured with a new webhook with the payload URL `http://url-of-tailor-instance/hook` and just the "Pull request" event.
