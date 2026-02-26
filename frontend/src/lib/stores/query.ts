
interface CacheEntry<T> {
    data: T;
    timestamp: number;
}

class QueryStore {
    private cache = new Map<string, CacheEntry<any>>();
    private staleTime = 5 * 60 * 1000; // 5 minutes

    async fetch<T>(key: string, fetcher: () => Promise<T>): Promise<T> {
        const now = Date.now();
        const cached = this.cache.get(key);

        if (cached && (now - cached.timestamp < this.staleTime)) {
            return cached.data;
        }

        const data = await fetcher();
        this.cache.set(key, { data, timestamp: now });
        return data;
    }

    invalidate(key: string) {
        this.cache.delete(key);
    }
}

export const queryClient = new QueryStore();
