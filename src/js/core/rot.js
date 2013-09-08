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

