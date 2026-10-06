# ADR 0029: checked shift counts and float-to-integer conversions

Status: accepted for native scalar completion after phase 7.

Shifts accept integer operands, including differently typed counts already allowed
by the frontend. Negative counts and counts greater than or equal to the left
operand's bit width abort. Left shifts operate on the fixed-width bit pattern,
discarding bits shifted out; they do not perform arithmetic overflow checking.
Right shifts of signed integers sign-extend; unsigned right shifts zero-fill.
The result has the left operand's type. Counts are checked before Cranelift shift
instructions, whose masking behavior must never define Tarn policy.

Float-to-integer casts truncate toward zero, then check the resulting integer
against the exact destination range. NaN, either infinity and out-of-range results
abort. Signed destinations require `-2^(bits-1) <= trunc(x) < 2^(bits-1)`;
unsigned destinations require `0 <= trunc(x) < 2^bits`. Thus u8(-0.5) is 0,
i8(127.9) is 127 and i8(-128.9) is -128. Checks use the source float width and
exact power-of-two bounds; checking a rounded maximum integer would accept the
exclusive upper boundary incorrectly. Only after these checks does codegen emit
the native conversion and any integer narrowing. No saturating conversion is
silently adopted. f32 and f64 follow the same policy.

These faults use the existing abort-only runtime ABI. There is no unwinding,
cleanup-on-fault or new runtime helper. Tests exercise fixed-width signed/unsigned
shifts, invalid counts, fractions, negative unsigned fractions, range failures,
NaN and infinity. The policy favors explicit checked boundaries while preserving
useful bit-pattern shifts and predictable truncating casts.
