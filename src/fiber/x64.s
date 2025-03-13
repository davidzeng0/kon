.global xx_core_fiber_x64_start
xx_core_fiber_x64_start:
	mov rax, [rsp - 0x10]
	mov rdi, [rsp - 0x08]
	push qword ptr 0
	jmp rax

.global xx_core_fiber_x64_intercept
xx_core_fiber_x64_intercept:
	mov rax, [rsp - 0x18]
	mov rdi, [rsp - 0x10]
	sub rsp, 8
	jmp rax

.global xx_core_fiber_x64_switch
xx_core_fiber_x64_switch:
	mov rax, [rsp]
	add rsp, 8

	mov [rdi + 0x00], r12
	mov [rdi + 0x08], r13
	mov [rdi + 0x10], r14
	mov [rdi + 0x18], r15
	mov [rdi + 0x20], rax
	mov [rdi + 0x28], rsp
	mov [rdi + 0x30], rbx
	mov [rdi + 0x38], rbp

	mov rax, [rsi + 0x20]
	mov r12, [rsi + 0x00]
	mov r13, [rsi + 0x08]
	mov r14, [rsi + 0x10]
	mov r15, [rsi + 0x18]
	mov rsp, [rsi + 0x28]
	mov rbx, [rsi + 0x30]
	mov rbp, [rsi + 0x38]

	jmp rax
