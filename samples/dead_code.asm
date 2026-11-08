# Dead code, identity noise, and a multiply-by-one: a size-mode target.
# Observable: result = r0 ^ r1, locals[2] = r0 ^ r1.
    push 5
    pop                 # dead push/pop pair
    load 0
    load 1
    xor
    push 0x5a5a5a5a
    xor
    push 0x5a5a5a5a
    xor                 # x ^ k ^ k == x
    push 1
    mul                 # multiply by one
    store 2             # observable side effect: locals[2]
    load 2
    ret
