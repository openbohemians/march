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

