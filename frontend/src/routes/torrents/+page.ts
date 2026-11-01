import type { PageLoad } from './$types';

/**
 * The /torrents workspace loads everything client-side from qbitStore; the
 * (empty) universal load only marks the route as having a load function.
 */
export const load: PageLoad = () => {};
