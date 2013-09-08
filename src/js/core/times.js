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

