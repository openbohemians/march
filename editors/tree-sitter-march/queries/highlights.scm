; March highlighting. Patterns are kept mutually exclusive where they could
; overlap, so the result does not depend on an editor's precedence rule.

(comment) @comment
(string) @string
(number) @number

[":" ";"] @punctuation.delimiter
["[" "]" "(" ")" "{" "}"] @punctuation.bracket

; Headings name namespaces.
(heading marker: (heading_marker) @punctuation.special)
(heading name: (word) @title)

; Definitions: capitalized names are types (Money, Config), others are words.
(definition name: (word) @type (#match? @type "^[A-Z]"))
(definition name: (word) @function (#not-match? @function "^[A-Z]"))
(system_definition name: (word) @function)
(entry name: (word) @property)

; Context lines: types, guards (ending in ?), type and row variables
; (ending in ' or '*), and -> between inputs and outputs.
(context marker: (context_marker) @keyword)
(context (word) @operator (#eq? @operator "->"))
(context (word) @variable.special (#match? @variable.special "'\\*?$"))
(context (word) @function (#match? @function "\\?$"))
(context (word) @type (#not-match? @type "(^->$)|('\\*?$)|(\\?$)"))
(context (_ (word) @variable.special (#match? @variable.special "'\\*?$")))
(context (_ (word) @operator (#eq? @operator "->")))
(context (_ (word) @type (#not-match? @type "(^->$)|('\\*?$)")))

; Control words.
((word) @keyword
  (#match? @keyword "^(if|else|then|while|times|match|undo|recur|exit|cycle|repeat|again|until|immediate)$"))

; .name reads an entry of the map on top; _ takes a value from below a
; bracket; i0 and i1 are loop indices; :x binds a local.
((word) @property (#match? @property "^\\.[^.]"))
((word) @variable.special (#match? @variable.special "^(_|i0|i1)$"))
((word) @variable (#match? @variable "^:[^:]"))
