PROJECT: March
VERSION: α₄

March is a progrmming language built from the ground up --
from machine code to an ultra-modern high-productivity development system.

FORTH is first and foremost inspiration. When in doubt, it never hurts to ask "What would Chuck do?".

A program is stored in an SQLite database. But also can have textual representations for working in a traditional manner.

Privimative words are machine code (we are using NASM assembly -- but I imagine we can use anything that can spit our machine code.

Primative words are stored with a constant index key -- we considered using a content-hash of the machine code, but it would require a recompile.
And a small primitive (16-bit) speeds up the interpretor. Primative machine code is also index by architechure.
Other architectures can be supported simply by writing the necessary primatives in an architecures machine code.

User words are stored with an index key that is content-base id, a CID, which is the CID of the CIDs that encode it's definition.

Literals are encoded (serialized) and stored with a CID as well.

I am running CahyOS Liunx on Intel Core Ultra 7. (Obviously our first target architecture.)

The core model is essentially FORTH. There is a data stack and a return stack, and an inner iterprestor. THere is also an outer-interpretor,
but it rather basic presently.

When compiled, a program is read from the database and "stiched" together, in much the same way as FORTH reads from a token stream.

See DESIGN.md document for details.

