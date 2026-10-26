/// 与之前一版不一样的原因：
/// 1.存在两个需要保存的东西，一个是内核栈的栈顶指针，还有一个就是内核地址空间的token需要保存进satp中
/// sscratch作为特殊的寄存器可以用来保存内核栈指针，但是内核地址空间的token没有办法保存，如果按照之前的办法将trap上下文保存进内核栈的话，需要首先切换为内核的地址空间，那用户的地址空间token就无法保存
/// 所以采用了更加优化的办法，sscratch和sp继续交换，sscratch保存的是trap上下文的指针，先不更换地址空间，将trap上下文保存在用户地址空间的次高页，并且统一所有的用户程序，再切换到内核的地址空间
///
/// 2.trap陷入和切出指令统一放在trampoline跳板也的原因：
/// 在trap陷入指令的时候需要切换内核和用户的token，放在最高页恒等映射在切换satp的值的时候，pc所在的真实地址不会应为页表的映射不同而改变
.altmacro
.macro SAVE_GP n
    sd x\n, \n*8(sp)
.endm
.macro LOAD_GP n
    ld x\n, \n*8(sp)
.endm
    .section .text.trampoline
    .globl __alltraps
    .globl __restore
    .align 2
__alltraps:
    csrrw sp, sscratch, sp
    # now sp->*TrapContext in user space, sscratch->user stack
    # save other general purpose registers
    sd x1, 1*8(sp)
    # skip sp(x2), we will save it later
    sd x3, 3*8(sp)
    # skip tp(x4), application does not use it
    # save x5~x31
    .set n, 5
    .rept 27
        SAVE_GP %n
        .set n, n+1
    .endr
    # we can use t0/t1/t2 freely, because they have been saved in TrapContext
    csrr t0, sstatus
    csrr t1, sepc
    sd t0, 32*8(sp)
    sd t1, 33*8(sp)
    # read user stack from sscratch and save it in TrapContext
    csrr t2, sscratch
    sd t2, 2*8(sp)
    # load kernel_satp into t0
    ld t0, 34*8(sp)
    # load trap_handler into t1
    ld t1, 36*8(sp)
    # move to kernel_sp
    ld sp, 35*8(sp)
    # switch to kernel space
    csrw satp, t0
    sfence.vma
    # jump to trap_handler
    jr t1

__restore:
    # a0: *TrapContext in user space(Constant); a1: user space token
    # switch to user space
    csrw satp, a1
    sfence.vma
    csrw sscratch, a0 ///sscratch的初始化
    mv sp, a0
    # now sp points to TrapContext in user space, start restoring based on it
    # restore sstatus/sepc
    ld t0, 32*8(sp)
    ld t1, 33*8(sp)
    csrw sstatus, t0
    csrw sepc, t1
    # restore general purpose registers except x0/sp/tp
    ld x1, 1*8(sp)
    ld x3, 3*8(sp)
    .set n, 5
    .rept 27
        LOAD_GP %n
        .set n, n+1
    .endr
    # back to user stack
    ld sp, 2*8(sp)
    sret

    .section .data
    # emergency stack for kernel trap
    # in order to print trap info even if the kernel stack is corrupted.
__emergency:
    .align 4
    .space 1024 * 4
__emergency_end:


    .section .text
    .globl __trap_from_kernel
    # 2^2=4 bytes aligned for stvec
    .align 2
__trap_from_kernel:
    la sp, __emergency_end
    j trap_from_kernel    