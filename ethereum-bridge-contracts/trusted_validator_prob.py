import math
from prettytable import PrettyTable


def C(n, m):
    return math.factorial(n) // (math.factorial(m) * math.factorial(n - m))

# def prob(N, K, n, k):
#     return (C(K, k) * C(N - K, n - k)) / C(N, n)


def prob(all, untrusted, needed):
    return C(untrusted, needed) / C(all, needed)


ACTIVE_VALIDATORS = 42
VALIDATORS_THRESHOLD = ACTIVE_VALIDATORS - (ACTIVE_VALIDATORS - 1) // 3
MAX_HACKED_VALIDATORS = VALIDATORS_THRESHOLD
MAX_NEEDED_SIGNARURES = 25
MIN_NEEDED_SIGNARURES = 5

print("Active validators:", ACTIVE_VALIDATORS)
print("Validators threshold:", VALIDATORS_THRESHOLD)

first_row = ["Hacked validators"]
for i in range(MIN_NEEDED_SIGNARURES, MAX_NEEDED_SIGNARURES + 1):
    first_row.append(f'Sigs {i}')
table = PrettyTable(first_row)
for hacked_validators in range(1, MAX_HACKED_VALIDATORS + 1):
    row = [hacked_validators]
    for need_signatures in range(MIN_NEEDED_SIGNARURES, MAX_NEEDED_SIGNARURES + 1):
        if need_signatures > hacked_validators:
            row.append("0")
        else:
            res = prob(VALIDATORS_THRESHOLD,
                       hacked_validators, need_signatures)
            row.append(f'{res:.2e}')
    table.add_row(row)

print(table)
