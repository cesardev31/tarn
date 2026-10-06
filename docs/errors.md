# Diagnostics (draft 0)

Diagnostics are part of the product. Every diagnostic has:

| Field | Meaning |
|-------|---------|
| `code` | Stable identifier: `E` + 4 digits for errors, `W` + 4 digits for warnings |
| `severity` | `error`, `warning`, `note` |
| `kind` | snake_case machine name (e.g. `use_after_move`) |
| `message` | one line, lower case, no trailing period |
| `file`, `line`, `column` | 1-based position of the primary span |
| `labels` | spans with short messages (primary and secondary) |
| `notes` | extra facts ("`buffer` was moved on line 12") |
| `help` | an actionable suggestion, when one exists |

## Code ranges

| Range | Phase |
|-------|-------|
| E0001–E0999 | lexer |
| E1001–E1999 | parser |
| E2001–E2999 | name resolution |
| E3001–E3999 | types |
| E4001–E4099 | moves |
| E4101–E4199 | borrows |
| E4201–E4299 | lifetimes / reference escapes |
| E9001–E9999 | driver, I/O, backend |

Codes are never reused. `tarn explain E4001` (future) prints the long form from
`docs/errors/E4001.md`.

## Text format

```
error[E4001]: use of moved value `buffer`
  --> src/main.tarn:13:11
   |
12 |     b := buffer
   |          ------ value moved here
13 |     print(buffer)
   |           ^^^^^^ value used here after move
   |
   = note: `Buffer` is not a copy type
   = help: borrow it instead: `b := &buffer`
```

## JSON format

`tarn check --json` prints one JSON object per line (JSON Lines) so tools can
stream them:

```json
{"code":"E4001","severity":"error","kind":"use_after_move","message":"use of moved value `buffer`","file":"src/main.tarn","line":13,"column":11,"labels":[{"file":"src/main.tarn","line":13,"column":11,"end_line":13,"end_column":17,"primary":true,"message":"value used here after move"}],"notes":["`Buffer` is not a copy type"],"help":"borrow it instead: `b := &buffer`"}
```

## Registered codes

| Code | Kind | Message |
|------|------|---------|
| E0001 | `unexpected_character` | unexpected character `X` |
| E0002 | `unterminated_string` | unterminated string literal |
| E0003 | `invalid_escape` | invalid escape sequence |
| E0004 | `invalid_number` | invalid numeric literal |
| E0005 | `number_out_of_range` | integer literal is too large |
| E1001 | `unexpected_token` | expected X, found Y |
| E1002 | `expected_expression` | expected expression, found X |
| E1003 | `expected_type` | expected type, found X |
| E1004 | `expected_pattern` | expected pattern, found X |
| E1005 | `expected_statement_end` | expected end of statement, found X |
| E1006 | `chained_comparison` | comparison operators cannot be chained |
| E1007 | `compound_assignment` | compound assignment `+=` is not supported |
| E1008 | `invalid_assignment_target` | cannot assign to this expression |
| E1009 | `struct_literal_in_condition` | struct literal not allowed here |
| E1010 | `unclosed_delimiter` | unclosed delimiter `{` |
| E1011 | `missing_function_body` | function `f` has no body |
| E1012 | `unexpected_function_body` | interface methods cannot have a body |
| E1013 | `expected_item` | expected item, found X |
| E1014 | `misplaced_receiver` | `self` is only allowed as the first parameter of a method |
| E1015 | `invalid_pub` | `pub` is not allowed here |
| E1016 | `else_on_new_line` | `else` must be on the same line as `}` |
| E1017 | `chained_range` | range operators cannot be chained |
| E1018 | `missing_type_colon` | a type annotation on a binding needs `:` |
| E2001 | `undefined_name` | cannot find `x` in this scope |
| E2002 | `duplicate_definition` | `x` is defined more than once |
| E2003 | `duplicate_in_scope` | `x` is already declared in this scope |
| E2004 | `used_before_declaration` | `x` is used before its declaration |
| E2005 | `no_such_member` | enum `E` has no member `x` |
| E2006 | `private_item` | `x` is private to module `m` |
| E2007 | `module_not_found` | cannot find module `m` |
| E2008 | `not_a_type` | expected a type, found function `f` |
| E2010 | `ambiguous_method` | `m` is implemented by more than one interface for this type |
| E2011 | `invalid_method_owner` | cannot declare method `m` on interface `I` |
| E2012 | `not_an_interface` | expected an interface in `impl`, found struct `S` |
| E2013 | `not_an_interface_member` | `m` is not a method of interface `I` |
| E2014 | `missing_interface_method` | `impl I` is missing `m` |
| E2015 | `self_outside_method` | `self` is only available in methods with a receiver |
| E2016 | `not_a_struct` | expected a struct, found variant `E.V` |
| E2017 | `variant_name_case` | variant `v` must start with an uppercase letter |
| E2018 | `expected_variant` | expected a variant, found `f` |
| W2001 | `confusing_shadow` | `x` is shadowed in an inner scope and used again after it |
| W2002 | `shadows_item` | `x` shadows the builtin function `x` |
| W2003 | `unused_import` | unused import `m` |
| E4201 | `reference_escapes` | returned reference may outlive its owner (planned) |
| E4202 | `ambiguous_provenance` | cannot infer where the returned reference comes from (planned) |
| E4203 | `reference_in_struct` | struct fields cannot hold references (planned) |
| E4204 | `provenance_mismatch` | implementation returns a borrow its interface does not allow (planned) |

Golden tests in `tests/**/fail/*.tarn` pin the rendered text of each code.
