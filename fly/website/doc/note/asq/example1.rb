

# __pop.eye__ is a stack based language.

# The simplist action in __pop.eye__ is to push data onto
# the work stack. In __pop.eye__ all data is a kept in
# lists. This is very much like lisp. Here is the simplest
# example:

  > "Hello".

# Lets see what the stack holds,

  > look.
  ("Hello")

# Yep, we pushed a list of of one element, the string "Hello"
# onto the top of the work stack. We can push another:

  > "World".
  > look.
  ("Hello")("World")

# Now there are two lists, each with a single element on the
# the work stack. We can merge them by pushing a command
# on the the stack.

  > merge.

# By default merge cobines the top two lists on the stack into one,
# so now the work stack looks like this:

   ("Hello" "World").

# Merge is more flexible then this and is pushed with a preceeding
# integer, will merge the many items off the top of the stack,
# or you can tell it too merge all.

  > all merge.

# or

  > merge all.

# both are recognized statments. With two string elements in the top
# stack list is it possible to "merge" them into one string? Easily,
# at any time, you can actually drop down into the top list
# as if it were the work stack --we call this a substack. We use the
# statement 'drop.' to do this.

  > drop. merge. out.

# And 'out.', to get back out agian.
# Now lets have some fun. First lets clear the stack.

  > all pop.  ;; or pop all.

# Now lest add an empty list to it. We do this with a double dot.

  > ..
  > look.
  (())

# Now lets drop into it.

  > drop.

# And add some content.

  > "Red Fish" "Blue Fish".

# Now we have list in a list. Look at the stack

  > look.
  (("Red Fish" "Blue Fish"))

# Notice the parens. Just like lisp, __pop.eye__ keeps track of list depth.

  > "One Fish" "Two Fish".
  > look.
  (("Red Fish" "Blue Fish")("One Fish" "Two Fish"))

  > up.
  > "nextpage".
  > look.
  (("Red Fish" "Blue Fish")("One Fish" "Two Fish"))(nextpage)



"Hello World!" print.

# :print sends a copy of the top of the work stack to
# the console "stack".


