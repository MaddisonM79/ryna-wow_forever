/** The agent's version, from package.json: a snapshot carries it, and a new one re-pushes each world's newest save once. */

import pkg from "../package.json" with { type: "json" };

export const VERSION: string = (pkg as { version: string }).version;
