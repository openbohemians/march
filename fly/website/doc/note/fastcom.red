<html>
<head>
  <title>Asqting</title>
</head>
<body>

<h1>Asqting<h1>

<p>Asqting is an interesting language. It's based largely on Forth, in that it is a stack based procedural language.
But unlike Forth it uses many fewer crytpic words, has types a rather then stacks of words, has stacks of clauses.
The later allows for statments to be made in non-reverse notation, though the core of the language
sticks with reverse notation to emphesize the stack nature of the language.</p>
 
<p>Template for building a new word.</p>

<pre>
  def {<i>type</i>|<i>word</i>}+ [ <i>block</i> ] .
</pre>


<h2>Built in Types</h2>
<pre>
  Number
  Integer
  Word
  String
  Block
</pre>

<h2>Built in Words<h2>

<h3>Special Words</h3>
<pre>
    define, :
    type
</pre>

<h3>Bitwise Operators</h3>
<pre>
  Value not, ~
  Value Value and, **
  Value Value or, ++
  Value Value xor, ^^
  Value Value nand, ~**
  Value Value nor, ~++
  Value Value xnor, ~^^
</pre>

<h3>Mathematics</h3>
<pre>
  Value* add, +
  Value Value sub, ~
  Value* mul, *
  Value Value div, /
  Value Value pow, ^
  Value Value log
  Value Value root
  Value Value exp
  Value Value mod, |
</pre>

<h3>Input/Output</h3>
<pre>
  Block DataStore write
  Block DataStore read
<pre>

<h3>Assembly</h3>
<pre>
  Value Register stor.
  Value Address stor.
  Register Register swap.
  Address Address swap.
  Register Register add.
  Address Address add.
</pre>

<h3>Conditionals</h3>
<pre>
  Block -: Block .
  if Block then Block { elsif Block then Block }* { else Block } .
</pre>

<h3>Equality</h3>
<pre>
  Value Value eql ?
  Value Value = ?

  Value Value gt ?
  Value Value > ?

  Value Value lt ?
  Value Value < ?

  Value Value gte ?
  Value Value >= ?

  Value Value lte ?
  Value Value <= ?

  Value Value ~= ?
  Value Value neq ?
</pre>

<h2>Examples</h2>

<pre>
  : Integer Integer multiple? [
    mod. 0 eql?
  ]
  > method multiple?

  4 2 multiple?
  > true.

  define Integer + Integer {
    add.
  end

  2 + 2.
  > 4.
</pre>
  
</body>
</html>
