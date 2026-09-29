# Generation-zero instruction listing. These are March words expressed as
# instructions before a March source reader exists. No Rust compiler handlers.
# Region 1 is FORTH working memory; private offsets are defined in SLICE.md.
entry boot
root empty
data empty

word get
lit 1
prim swap
prim load64
ret
end

word put
lit 1
prim swap
prim store64
ret
end

word inc-pos
lit 56
call get
lit 1
prim add
lit 56
call put
ret
end

word input-char
lit 40
call get
lit 56
call get
prim load8
ret
end

word copy
lit 128
call put
lit 136
call put
lit 144
call put
lit 152
call put
lit 160
call put
loop:
lit 128
call get
zero done
lit 160
call get
lit 152
call get
prim load8
lit 144
call get
lit 136
call get
prim store8
lit 152
call get
lit 1
prim add
lit 152
call put
lit 136
call get
lit 1
prim add
lit 136
call put
lit 128
call get
lit 1
prim sub
lit 128
call put
branch loop
done:
ret
end

word install
lit 176
call put
lit 184
call put
lit 192
call put
lit 200
call put
lit 208
call put
lit 176
call get
prim dup
zero bad
lit 256
prim lt
zero bad
lit 32
call get
lit 216
call put
lit 8
call get
lit 216
call get
call put
lit 208
call get
lit 216
call get
lit 8
prim add
call put
lit 200
call get
lit 216
call get
lit 16
prim add
call put
lit 176
call get
lit 216
call get
lit 24
prim add
call put
lit 192
call get
lit 184
call get
lit 1
lit 216
call get
lit 32
prim add
lit 176
call get
call copy
lit 216
call get
lit 32
prim add
lit 176
call get
prim add
lit 7
prim add
lit -8
prim and
lit 32
call put
lit 216
call get
lit 8
call put
ret
bad:
lit 7
prim trap
end

word find
lit 256
call put
lit 248
call put
lit 240
call put
lit 8
call get
lit 264
call put
next:
lit 264
call get
prim dup
zero done
lit 24
prim add
call get
lit 256
call get
prim eq
zero miss
lit 0
lit 272
call put
chars:
lit 272
call get
lit 256
call get
prim lt
zero found
lit 240
call get
lit 248
call get
lit 272
call get
prim add
prim load8
lit 1
lit 264
call get
lit 32
prim add
lit 272
call get
prim add
prim load8
prim eq
zero miss
lit 272
call get
lit 1
prim add
lit 272
call put
branch chars
miss:
lit 264
call get
call get
lit 264
call put
branch next
found:
lit 264
call get
ret
done:
ret
end

word token
lit 40
call get
lit 64
call get
lit 72
call get
ret
end

word read-word
skip:
lit 56
call get
lit 48
call get
prim lt
zero eof
call input-char
lit 33
prim lt
zero start
call inc-pos
branch skip
start:
lit 56
call get
lit 64
call put
scan:
lit 56
call get
lit 48
call get
prim lt
zero done
call input-char
lit 33
prim lt
zero consume
branch done
consume:
call inc-pos
branch scan
done:
lit 56
call get
lit 64
call get
prim sub
lit 72
call put
lit 1
ret
eof:
lit 0
lit 72
call put
lit 0
ret
end

word word
call read-word
zero bad
call token
ret
bad:
lit 4
prim trap
end

word scan-char
lit 40
call get
lit 288
call get
prim load8
ret
end

word scan-inc
lit 288
call get
lit 1
prim add
lit 288
call put
ret
end

word number
lit 64
call get
lit 288
call put
lit 64
call get
lit 72
call get
prim add
lit 296
call put
lit 0
lit 304
call put
call scan-char
lit 45
prim eq
zero plus
lit 1
lit 304
call put
call scan-inc
branch sign-done
plus:
call scan-char
lit 43
prim eq
zero sign-done
call scan-inc
sign-done:
lit 288
call get
lit 296
call get
prim eq
zero nonempty
branch no
nonempty:
lit 288
call get
lit 336
call put
validate:
lit 288
call get
lit 296
call get
prim lt
zero calculate
call scan-char
lit 48
prim sub
lit 10
prim lt
zero no
call scan-inc
branch validate
calculate:
lit 336
call get
lit 288
call put
lit 0
lit 312
call put
lit 9223372036854775807
lit 304
call get
prim add
lit 320
call put
digits:
lit 288
call get
lit 296
call get
prim lt
zero finish
call scan-char
lit 48
prim sub
lit 328
call put
lit 320
call get
lit 328
call get
prim sub
lit 10
prim div
lit 312
call get
prim lt
zero safe
lit 2
prim trap
safe:
lit 312
call get
lit 10
prim mul
lit 328
call get
prim add
lit 312
call put
call scan-inc
branch digits
finish:
lit 312
call get
lit 304
call get
zero positive
lit 0
prim swap
prim sub
positive:
lit 1
ret
no:
lit 0
ret
end

word c-comma
lit 24
call get
lit 16
call get
prim store8
lit 16
call get
lit 1
prim add
lit 16
call put
ret
end

word comma
lit 352
call put
lit 0
lit 360
call put
loop:
lit 360
call get
lit 8
prim lt
zero done
lit 352
call get
lit 360
call get
lit 8
prim mul
prim shr
call c-comma
lit 360
call get
lit 1
prim add
lit 360
call put
branch loop
done:
ret
end

word compile-call
lit 3
call c-comma
lit 24
call get
lit 16
call get
prim code-cid
lit 16
call get
lit 32
prim add
lit 16
call put
ret
end

word compile-quote
lit 4
call c-comma
lit 24
call get
lit 16
call get
prim code-cid
lit 16
call get
lit 32
prim add
lit 16
call put
ret
end

word literal
lit 1
call c-comma
call comma
ret
end

word begin
lit 0
call get
zero ok
lit 5
prim trap
ok:
lit 88
call put
lit 80
call put
lit 96
call put
lit 65536
prim region-new
lit 24
call put
lit 0
lit 16
call put
lit 1
lit 0
call put
ret
end

word colon
call word
call begin
ret
end

word semicolon
lit 0
call get
zero bad
lit 0
call c-comma
lit 24
call get
lit 0
lit 16
call get
prim seal
lit 0
lit 96
call get
lit 80
call get
lit 88
call get
call install
lit 24
call get
prim region-free
lit 0
lit 24
call put
lit 0
lit 0
call put
ret
bad:
lit 6
prim trap
end

word immediate
lit 8
call get
prim dup
zero bad
lit 16
prim add
lit 1
prim swap
call put
ret
bad:
lit 7
prim trap
end

word to-xt
lit 8
prim add
call get
ret
end

word quote-word
call word
call find
prim dup
zero bad
call to-xt
lit 0
call get
zero done
call compile-quote
done:
ret
bad:
lit 1
prim trap
end

word emit-recur
lit 7
call c-comma
ret
end

word line-comment
loop:
lit 56
call get
lit 48
call get
prim lt
zero done
call input-char
call inc-pos
lit 10
prim eq
zero loop
done:
ret
end

word evaluate
lit 48
call put
lit 40
call put
lit 0
lit 56
call put
loop:
call read-word
zero done
call number
zero lookup
lit 0
call get
zero loop
call literal
branch loop
lookup:
call token
call find
prim dup
zero unknown
prim dup
lit 16
prim add
call get
zero ordinary
branch execute
ordinary:
lit 0
call get
zero execute
call to-xt
call compile-call
branch loop
execute:
call to-xt
prim execute
branch loop
unknown:
lit 1
prim trap
done:
lit 0
call get
zero finish
lit 3
prim trap
finish:
ret
end

word recover
lit 24
call get
prim dup
zero no-builder
prim region-free
branch reset
no-builder:
prim drop
reset:
lit 0
lit 24
call put
lit 0
lit 16
call put
lit 0
lit 0
call put
ret
end

word out-byte
lit 400
call get
lit 408
call get
prim store8
lit 408
call get
lit 1
prim add
lit 408
call put
ret
end

word out-cell
lit 352
call put
lit 0
lit 360
call put
loop:
lit 360
call get
lit 8
prim lt
zero done
lit 352
call get
lit 360
call get
lit 8
prim mul
prim shr
call out-byte
lit 360
call get
lit 1
prim add
lit 360
call put
branch loop
done:
ret
end

word save-list
prim dup
zero empty
prim dup
call get
recur
lit 440
call put
lit 440
call get
lit 24
prim add
call get
call out-cell
lit 440
call get
lit 16
prim add
call get
call out-cell
lit 440
call get
call to-xt
lit 400
call get
lit 408
call get
prim code-cid
lit 408
call get
lit 32
prim add
lit 408
call put
lit 1
lit 440
call get
lit 32
prim add
lit 400
call get
lit 408
call get
lit 440
call get
lit 24
prim add
call get
call copy
lit 408
call get
lit 440
call get
lit 24
prim add
call get
prim add
lit 408
call put
ret
empty:
prim drop
ret
end

word snapshot
lit 1048576
prim region-new
lit 400
call put
lit 0
lit 408
call put
lit 8
call get
call save-list
lit 400
call get
lit 0
lit 408
call get
ret
end

word in-cell
lit 0
lit 352
call put
lit 0
lit 360
call put
loop:
lit 360
call get
lit 8
prim lt
zero done
lit 384
call get
lit 392
call get
prim load8
lit 360
call get
lit 8
prim mul
prim shl
lit 352
call get
prim or
lit 352
call put
lit 392
call get
lit 1
prim add
lit 392
call put
lit 360
call get
lit 1
prim add
lit 360
call put
branch loop
done:
lit 352
call get
ret
end

word restore
lit 416
call put
lit 392
call put
lit 384
call put
loop:
lit 392
call get
lit 416
call get
prim lt
zero done
call in-cell
lit 424
call put
call in-cell
prim dup
lit 2
prim lt
zero bad
lit 432
call put
lit 384
call get
lit 392
call get
prim resolve
lit 392
call get
lit 32
prim add
lit 392
call put
lit 432
call get
lit 384
call get
lit 392
call get
lit 424
call get
call install
lit 392
call get
lit 424
call get
prim add
lit 392
call put
branch loop
done:
lit 392
call get
lit 416
call get
prim eq
zero bad
ret
bad:
lit 7
prim trap
end

word p_dup
prim dup
ret
end

word p_drop
prim drop
ret
end

word p_swap
prim swap
ret
end

word p_over
prim over
ret
end

word p_rot
prim rot
ret
end

word p_add
prim add
ret
end

word p_sub
prim sub
ret
end

word p_mul
prim mul
ret
end

word p_div
prim div
ret
end

word p_mod
prim mod
ret
end

word p_eq
prim eq
ret
end

word p_lt
prim lt
ret
end

word p_and
prim and
ret
end

word p_or
prim or
ret
end

word p_xor
prim xor
ret
end

word p_not
prim not
ret
end

word p_shl
prim shl
ret
end

word p_shr
prim shr
ret
end

word p_load8
prim load8
ret
end

word p_store8
prim store8
ret
end

word p_load64
prim load64
ret
end

word p_store64
prim store64
ret
end

word p_region-new
prim region-new
ret
end

word p_region-free
prim region-free
ret
end

word p_region-size
prim region-size
ret
end

word p_execute
prim execute
ret
end

word var_STATE
lit 1
lit 0
ret
end

word var_HERE
lit 1
lit 16
ret
end

word var_LATEST
lit 1
lit 8
ret
end

data name0 647570
data name1 64726f70
data name2 73776170
data name3 6f766572
data name4 726f74
data name5 752b
data name6 752d
data name7 752a
data name8 752f
data name9 756d6f64
data name10 65713f
data name11 756c743f
data name12 616e64
data name13 6f72
data name14 786f72
data name15 696e76657274
data name16 6c7368696674
data name17 727368696674
data name18 6340
data name19 6321
data name20 40
data name21 21
data name22 726567696f6e2d6e6577
data name23 726567696f6e2d66726565
data name24 726567696f6e2d73697a65
data name25 63616c6c
data name26 776f7264
data name27 66696e64
data name28 3e7874
data name29 626567696e
data name30 3a
data name31 3b
data name32 696d6d656469617465
data name33 6c69746572616c
data name34 636f6d70696c652c
data name35 71756f7465
data name36 27
data name37 2c
data name38 632c
data name39 2d2d
data name40 7265637572
data name41 5354415445
data name42 48455245
data name43 4c4154455354

word init
quote p_dup
lit 0
ref name0
call install
quote p_drop
lit 0
ref name1
call install
quote p_swap
lit 0
ref name2
call install
quote p_over
lit 0
ref name3
call install
quote p_rot
lit 0
ref name4
call install
quote p_add
lit 0
ref name5
call install
quote p_sub
lit 0
ref name6
call install
quote p_mul
lit 0
ref name7
call install
quote p_div
lit 0
ref name8
call install
quote p_mod
lit 0
ref name9
call install
quote p_eq
lit 0
ref name10
call install
quote p_lt
lit 0
ref name11
call install
quote p_and
lit 0
ref name12
call install
quote p_or
lit 0
ref name13
call install
quote p_xor
lit 0
ref name14
call install
quote p_not
lit 0
ref name15
call install
quote p_shl
lit 0
ref name16
call install
quote p_shr
lit 0
ref name17
call install
quote p_load8
lit 0
ref name18
call install
quote p_store8
lit 0
ref name19
call install
quote p_load64
lit 0
ref name20
call install
quote p_store64
lit 0
ref name21
call install
quote p_region-new
lit 0
ref name22
call install
quote p_region-free
lit 0
ref name23
call install
quote p_region-size
lit 0
ref name24
call install
quote p_execute
lit 0
ref name25
call install
quote word
lit 0
ref name26
call install
quote find
lit 0
ref name27
call install
quote to-xt
lit 0
ref name28
call install
quote begin
lit 0
ref name29
call install
quote colon
lit 0
ref name30
call install
quote semicolon
lit 1
ref name31
call install
quote immediate
lit 0
ref name32
call install
quote literal
lit 0
ref name33
call install
quote compile-call
lit 0
ref name34
call install
quote quote-word
lit 1
ref name35
call install
quote quote-word
lit 1
ref name36
call install
quote comma
lit 0
ref name37
call install
quote c-comma
lit 0
ref name38
call install
quote line-comment
lit 1
ref name39
call install
quote emit-recur
lit 1
ref name40
call install
quote var_STATE
lit 0
ref name41
call install
quote var_HERE
lit 0
ref name42
call install
quote var_LATEST
lit 0
ref name43
call install
ret
end

word boot
lit 4096
lit 32
call put
lit 0
lit 8
call put
lit 0
lit 0
call put
prim dup
zero fresh
call restore
branch ready
fresh:
prim drop
prim drop
prim drop
call init
ready:
quote evaluate
quote recover
quote snapshot
ret
end

