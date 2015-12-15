//
// Remove the top item from the stack.
//
defword("drop", "a --", function() {
    pop();
});

//
//
//
defword("dup", "a -- a a", function() {
    var s1 = at(1);
    push(s1);
});

//
//
//
defcode("over", "a a -- a a a", function() {
    var s2 = at(2);
    push(s2);
});

//
// Rotate the top three stack items.
//
// Example
//   a b c rot  ->  c a b
//
defcode("rot", "a a a -- a a a", function() {
    var s1 = pop();
    var s2 = pop();
    var s3 = pop();
    push(s1);
    push(s3);
    push(s2);
});

//
// Unrotate the top three stack items.
//
// Example
//   a b c rot  ->  c b a
//
defcode("unrot", "a a a -- a a a", function() {
    var s1 = pop();
    var s2 = pop();
    var s3 = pop();
    push(s1);
    push(s2);
    push(s3);
});

//
// Swap the top two stack entries.
//
defcode("swap", "a a -- a a", function() {
    var s1 = pop();
    var s2 = pop();
    push(s1);
    push(s2);
});

//
// Top two are equal?
//
// Example
//     1 1.0 ==  -> true
//
defcode("=", "n n -- b", function() {
    s1 = pop();
    s2 = pop();
    push(s1 == s2)
});

//
// Top is less than the next?
//
// Example
//     1 2 <   -> true
//     3 2 <   -> false
//
defcode("<", "n n -- b", function() {
    s1 = pop();
    s2 = pop();
    push(s1 < s2)
});

//
// Top is greater than the next?
//
// Example
//     1 2 >   -> false
//     3 2 >   -> true
//
defcode(">", "n n -- b", function() {
    s1 = pop();
    s2 = pop();
    push(s1 > s2)
});

//
// Top is less than or equal to next?
//
// Example
//     1 2 =<   -> true
//     2 2 =<   -> true
//     3 2 =<   -> false
//
defcode("=<", "n n -- b", function() {
    s1 = pop();
    s2 = pop();
    push(s1 < s2)
});

//
// Top is greater than or equal to next?
//
// Example
//     1 2 >=   -> false
//     2 2 >=   -> true
//     3 2 >=   -> true
//
defcode(">=", "n n -- b", function() {
    s1 = pop();
    s2 = pop();
    push(s1 > s2)
});

//
// Top two words are stricty equal? In this case difference kinds of numbers
// are not considered equal, e.g. `1` and `1.0`.
//
// Example
//     1 1   ==  -> true
//     1 1.0 ==  -> false
//
// TODO: Rename to `same`?
//
defcode("==", "a a -- b", function() {
    s1 = pop();
    s2 = pop();

    push(getType(s1) == getType(s2) && s1 == s2)
});

//
// Monadic do-times.
//
// Example
//   A B C 2 times drop  -> A
//   A B C 2 times rot   -> B C A
//
defcode("times", "+int -- *", function() {
    s1 = pop();
    m = function(word) {
        for(i = 0; i < s1; i++)
           interpret(word);
        }
    }
    push(m);
});

//
// Alias for monadic do-times.
//
// Example
//   A B C 2 x drop  -> A
//   A B C 2 x rot   -> B C A
//
aliascode("x", "times");

