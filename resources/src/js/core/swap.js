//
// Swap the top two stack entries.
//

defcode("swap", "a a -- a a", function() {
    var s1 = pop();
    var s2 = pop();
    push(s1);
    push(s2);
});

