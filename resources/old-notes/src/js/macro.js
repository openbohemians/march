// Javascript Macros

var AVLTree = require('binary-search-tree').AVLTree;

//
// We keep the dictrionary as an balanced binary search tree.
// This makes it fast O(log n) to lookup words.
//

var dictionary = new AVLTree();

//
// TODO: Bet data structure for this?
//

var contextStack = new Array();

//
// TODO: Bet data structure for this?
//

var dataStack = new Array();

//
// TODO: Best data strcuture for this?
//

var returnStack = new Array();

//
// Constants table.
//
// Unlike other languages, this constant table can be written to during
// runtime. But everything added to it becomes immutable/read-only.
//

var constants = {
    'version': '0.0.0'
};


//
// Word type.
//
function Word(name, type, code) {
    this.name     = name;
    this.typedefs = [];
    this.add(type, code);
}

//
// Add a typedef to the word.
//
Word.prototype.add = function(type, code) {
    parts = type.trim().split("\s+");
    this.typedefs.push([type, code]);
}

//
// Define a new word.
//
function defword(name, type, definition) {
    name = validName(type);
    type = validType(type);
    code = compile(definition);  

    var word;

    word = dictionary.search(name);

    if (word) {
        validateArity(word, type);
        word.add(type, code);
    } else {
        word = new Word(name, type, code);
        dictionary.insert(name, word);
    }
}

//
// Define core word in implementation language, javascript in this case.
//
function defcode(name, type, lambda) {
    name = validName(type);
    type = validType(type);

    var word;

    word = dictionary.search(name);

    if (word) {
        validateArity(word, type);
        word.add(type, lambda);
    } else {
        word = new Word(name, type, lambda);
        dictionary.insert(name, word);
    }
}

//
// Compile a word definition. This convert words into pointers which reference
// the word defintions in the dictionary.
//
function compile(definition) {
    precode = parseDef(definition);

    // should be build a AST instead?
    var code = [];

    for(i = 0; i < precode.size; i++) {
        word = precode[i]
        // TODO: handle literals

        // handle fuinction calls
        codeword = dictionary.search(word);
        if (typeCode) {
            code.push(codeword);
        } else {
            compileError("underfined word: " + word);
        }
    }

    return code;
}

//
// Takes a definition and returns array of words.
//
// TODO: Using an intermediate link list instead of an array might speed this up.
//
function parseDef(definition) {
    var words = new Array();
    for(i = 0; i < definition.size; i++) {
        parseWord(definition[i], words) {
    }
    return words;
}

//
//
//
function parseWord(word, words) {
    n = word.size - 1;
    b = word.charAt(0);
    e = word.charAt(n);

    // beginning
    switch(b) {
    case '[':
    case '{':
    case '(':
        words.push(e)
        word = word.substring(0,n-1)
        parseWord(word, words)
        break;
    default:
        // ending
        switch(e) {
        case ']':
        case '}':
        case ')':
            word = word.substring(0,n-1)
            words.push(e)
            break;
        default:
            // all done
            words.push(word)
        }
    }  
}

//
// Ensures any new word definition has the same arity as any previous
// definition.
//
function validateArity(word, type)
    predef_type = word[0][0]

    // TODO
    return true
}

////////////////////////////
// STACK HELPER FUNCTIONS //
////////////////////////////

//
//
//
function pop() {
  return stack.pop();
}

//
//
//
function push(o) {
  return stack.push(o);
}

//
//
//
function at(i) {
  stack[stack.length - i]
}

//
//
//
function interpret(code)
    stack = code;

    word = stack.pop();

    arity = word.arity();

    typeface = [];
    for(var i = 1; i <= arity; i++) {
        typeface.push(typeOf(at(i)))
    }
}

//
//
//
function typeOf(word) {

}

