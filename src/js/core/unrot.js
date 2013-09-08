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

