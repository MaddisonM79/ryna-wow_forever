/** GroupSaveDataMap: guilds and the other character groups. A port of group.py. */

import type { CustomReader, FArchiveReader, Node } from "../gvas/reader.js";
import { EOF_NOT_REACHED, byte, guid, instanceId, node, nodes } from "./common.js";

function playerInfo(r: FArchiveReader): Node {
  return { player_uid: r.guid(), player_info: { last_online_real_time: r.i64(), player_name: r.fstring() } };
}

function guildPlayerInfo(r: FArchiveReader): Node {
  const p = playerInfo(r);
  p["role"] = r.byte(); // EPalGuildRole
  return p;
}

/** FPalGuildMarkerData: 60 bytes. */
function guildMarker(r: FArchiveReader): Node {
  return { marker_id: r.guid(), icon_location: r.vectorDict(), icon_type: r.i32(), owner_player_uid: r.guid() };
}

function rolePermission(r: FArchiveReader): Node {
  return { role: r.byte(), permissions: r.tarray(byte) };
}

/** Guild tail as of the 2026-07 update: chest roles, per-player role, permissions. */
function guildTailV2(r: FArchiveReader): Node {
  return {
    guild_chest_allowed_roles: r.tarray(byte),
    unknown_i32: r.i32(),
    admin_player_uid: r.guid(),
    players: r.tarray(guildPlayerInfo),
    role_permissions: r.tarray(rolePermission),
    trailing_bytes: r.byteList(4),
  };
}

/** Guild tail before the 2026-07 update. */
function guildTailV1(r: FArchiveReader): Node {
  return { admin_player_uid: r.guid(), players: r.tarray(playerInfo), trailing_bytes: r.byteList(4) };
}

/**
 * Pick the guild tail layout by which one consumes the blob exactly. There is
 * no version flag, so v2 is tried first and kept only if it lands on EOF.
 */
function guildTail(r: FArchiveReader): Node {
  const start = r.tell();
  try {
    const tail = guildTailV2(r);
    if (r.eof()) return tail;
  } catch {
    // not v2; fall through and try the pre-update layout
  }
  r.seek(start);
  return guildTailV1(r);
}

function groupBytes(parent: FArchiveReader, bytes: Uint8Array, groupType: string): Node {
  const r = parent.internalCopy(bytes);
  const data: Node = {
    group_type: groupType,
    group_id: r.guid(),
    group_name: r.fstring(),
    individual_character_handle_ids: r.tarray(instanceId),
  };
  if (["EPalGroupType::Guild", "EPalGroupType::IndependentGuild", "EPalGroupType::Organization"].includes(groupType)) {
    data["org_type"] = r.byte();
  }
  if (groupType === "EPalGroupType::Organization") data["trailing_bytes"] = r.byteList(12);
  if (groupType === "EPalGroupType::Guild") {
    data["leading_bytes"] = r.byteList(4);
    data["base_ids"] = r.tarray(guid);
    data["unknown_1"] = r.i32();
    data["base_camp_level"] = r.i32();
    data["map_object_instance_ids_base_camp_points"] = r.tarray(guid);
    data["guild_name"] = r.fstring();
    data["last_guild_name_modifier_player_uid"] = r.guid();
    data["guild_markers"] = r.tarray(guildMarker);
    Object.assign(data, guildTail(r));
  }
  if (groupType === "EPalGroupType::IndependentGuild") {
    data["base_camp_level"] = r.i32();
    data["map_object_instance_ids_base_camp_points"] = r.tarray(guid);
    data["guild_name"] = r.fstring();
    data["player_uid"] = r.guid();
    data["guild_name_2"] = r.fstring();
    data["player_info"] = { last_online_real_time: r.i64(), player_name: r.fstring() };
  }
  if (!r.eof()) throw new Error(EOF_NOT_REACHED);
  return data;
}

export const group: CustomReader = (reader, typeName, size, path) => {
  if (typeName !== "MapProperty") throw new Error(`Expected MapProperty, got ${typeName}`);
  const value = reader.property(typeName, size, path, path);
  for (const entry of nodes(value["value"])) {
    const v = node(entry["value"]);
    const groupType = node(node(v["GroupType"])["value"])["value"] as string;
    const raw = node(v["RawData"]);
    raw["value"] = groupBytes(reader, node(raw["value"])["values"] as Uint8Array, groupType);
  }
  return value;
};
