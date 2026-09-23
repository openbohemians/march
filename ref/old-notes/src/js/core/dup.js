defword("dup", "a -- a a", function() {
    var s1 = at(1);
    push(s1);
});

