; March highlighting. Patterns are kept mutually exclusive where they could
; overlap, so the result does not depend on an editor's precedence rule.

(comment) @comment
(string) @string
(number) @number

[":" ";"] @punctuation.delimiter
["[" "]" "(" ")" "{" "}" "<" ">"] @punctuation.bracket

; Headings name namespaces.
(heading marker: (heading_marker) @punctuation.special)
(heading name: (word) @title)

; Definitions: capitalized names are types (Money, Config), others are words.
(definition name: (word) @type (#match? @type "^[A-Z]"))
; Definitions of the control words themselves keep the keyword color.
(definition name: (word) @function (#not-match? @function "^[A-Z]") (#not-match? @function "^(if|else|then|while|times|match|undo|recur|exit|cycle|repeat|again|until|immediate)$"))
(system_definition name: (word) @function (#not-match? @function "^(if|else|then|while|times|match|undo|recur|exit|cycle|repeat|again|until|immediate)$"))
(entry name: (word) @property)

; Patterns, in context headings and named signatures: types, guards (ending
; in ?), type and row variables (ending in ' or '*), and -> between inputs and
; outputs.
(pattern (word) @operator (#eq? @operator "->"))
(pattern (word) @variable.special (#match? @variable.special "'\\*?$"))
(pattern (word) @function (#match? @function "\\?$"))
(pattern (word) @type (#not-match? @type "(^->$)|('\\*?$)|(\\?$)|(^=$)"))
(pattern (_ (word) @variable.special (#match? @variable.special "'\\*?$")))
(pattern (_ (word) @operator (#eq? @operator "->")))
(pattern (_ (word) @type (#not-match? @type "(^->$)|('\\*?$)")))

; = is dup and ~ is swap.
((word) @operator (#match? @operator "^[=~]$"))

; Control words.
((word) @keyword
  (#match? @keyword "^(if|else|then|while|times|match|undo|recur|exit|cycle|repeat|again|until|immediate)$"))

; .name reads an entry of the map on top; _ takes a value from below a
; bracket; i0 and i1 are loop indices; :x binds a local.
((word) @property (#match? @property "^\\.[^.]"))
((word) @variable.special (#match? @variable.special "^(_|i0|i1)$"))
((word) @variable (#match? @variable "^:[^:]"))
