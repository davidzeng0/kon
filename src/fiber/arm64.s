.global xx_core_fiber_arm64_start
xx_core_fiber_arm64_start:
	mov x30, xzr
	ldp x1, x0, [sp, #-0x10]
	br x1

.global xx_core_fiber_arm64_intercept
xx_core_fiber_arm64_intercept:
	ldp x1, x0, [sp, #-0x18]
	ldr x30, [sp, #-0x08]
	br x1

.global xx_core_fiber_arm64_switch
.align 16
xx_core_fiber_arm64_switch:
	mov x10, sp

	stp x19, x29, [x0, #0x00]
	stp x10, x30, [x0, #0x10]
	ldp x19, x29, [x1, #0x00]
	ldp x10, x30, [x1, #0x10]

	mov sp, x10
	br x30