/**
 * The file part names ingest (`POST /v1/palworld/agent/ingest`) accepts; anything else is refused
 * and never stored. A copy: its twin is RAW_NAME_RE in the private API's palworld service
 * (`packages/palworld-service/src/raw-names.ts`), which this public repository can't import. Change
 * both together, and the API first.
 */
export const RAW_NAME_RE = /^(?:Level|LevelMeta|WorldOption|LocalData)\.sav$|^Players\/[0-9A-Fa-f]{32}\.sav$|^fog\.png$/;
