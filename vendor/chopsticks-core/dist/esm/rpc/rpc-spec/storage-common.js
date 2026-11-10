export async function getDescendantValues(block, params) {
    const keys = await block.getKeysPaged({
        ...params,
        pageSize: PAGE_SIZE
    });
    const items = (await block.getMany(keys)).map((value, idx)=>({
            key: keys[idx],
            value
        }));
    if (keys.length < PAGE_SIZE) {
        return {
            items,
            next: null
        };
    }
    return {
        items,
        next: {
            ...params,
            startKey: keys[PAGE_SIZE - 1]
        }
    };
}
export const PAGE_SIZE = 1000;
export async function afterResponse(fn) {
    await new Promise((resolve)=>setTimeout(resolve, 0));
    fn();
}
