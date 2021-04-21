import json

with open('snapshot.json') as f:
    data = json.load(f, parse_float=lambda x: x)
referrers = {}
for entry in data:
    if 'invitees' in entry:
        for invitee in entry['invitees']:
            referrers[invitee] = entry['account']
with open('../../node/src/chain_spec/bytes/iroha_migration_accounts_staging.in', 'w') as f:
    print('{', file=f)
    print('    use common::balance;', file=f)
    print('    vec_push![', file=f)
    for entry in data:
        account = '"%s".to_string()' % entry['account']
        balance = 'balance!(%s)' % entry.get('balance', 0)
        referrer = referrers.get(entry['account'])
        if referrer is None:
            referrer = 'None'
        else:
            referrer = 'Some("%s".to_string())' % referrer
        public_keys = ', '.join(map(lambda k: '"%s".to_string()' % k, entry['pub_keys']))
        public_keys = 'vec![%s]' % public_keys
        print('        (', file=f)
        print('            %s,' % account, file=f)
        print('            %s,' % balance, file=f)
        print('            %s,' % referrer, file=f)
        print('            %s,' % entry['quorum'], file=f)
        print('            %s,' % public_keys, file=f)
        print('        ),', file=f)
    print('    ]', file=f)
    print('}', file=f)
