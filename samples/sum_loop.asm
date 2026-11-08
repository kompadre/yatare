# acc = 0; i = 1; while (i <= n) { acc += i; i += 1 }; return acc
# n arrives in locals[0] (all zero from `yatare run`, so the loop exits
# immediately and the program returns 0 with locals[2] = 1).
    load 0
    store 3             # limit = n
    push 0
    store 1             # acc = 0
    push 1
    store 2             # i = 1
loop:
    load 3
    load 2
    jlt exit            # n < i -> done (continue while i <= n)
    load 1
    load 2
    add
    store 1             # acc += i
    load 2
    push 1
    add
    store 2             # i += 1
    jmp loop
exit:
    load 1
    ret
