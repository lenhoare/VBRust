# A3 — Bank Account (Type + Handle)

A small bank-account demo that exercises Bust's **`Type` (struct)** and
error handling — the two features VB6 has no direct equivalent for (a struct
is like a VB6 `Type` that can also carry *methods*; errors propagate
automatically, and `Handle err` intercepts a call).

## What it does

Opens an account with a starting balance, deposits, withdraws, and tries an
overdraft — which is refused — printing the running balance after each step.
The logic lives in `bank.vbr` and is driven by a thin `main.vbr`.

## Bust language features tested

- `Public Type` / `End Type` — a struct with fields, built with the
  `Account { owner: ..., balance: ... }` literal constructor
- Methods on a type: `Function Account.Deposit(...)`, called on a value as
  `acc.Deposit(50)`
- `Me` as the receiver; `&mut self` is inferred automatically because the
  method body assigns to `Me.Balance`
- `RaiseError` to refuse an overdraft; a normal call propagates; `Handle err`
  intercepts at the call site
- A free helper (`TryWithdraw`) that turns a fallible withdraw into a `Boolean`
- `ByRef` parameters (`TryWithdraw(ByRef acc As Account, ...)`) — writes flow
  back to the caller

## Standard-library features tested

None — this project is pure core language.

## Running it

From the repository root:

```sh
vbr runproject projects/A3_bank_account    # build + run the program
vbr test        projects/A3_bank_account   # run the 4 tests
```

## Expected output

```
A3: bank account (Type + Result)

opened  Ada: 100p
deposit 50  -> 150
withdraw 30 -> 120
withdraw 200 failed: insufficient funds

final  Ada: 120p
```
