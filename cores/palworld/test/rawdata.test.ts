import { describe, expect, it } from "vitest";
import { FArchiveReader } from "../src/gvas/reader.js";
import { concreteModelBytes } from "../src/rawdata/concrete-model.js";
import { workBytes } from "../src/rawdata/work.js";

const b64 = (s: string): Uint8Array => new Uint8Array(Buffer.from(s, "base64"));
const reader = new FArchiveReader(new Uint8Array());

// The Ancient Hatchery on the 2026-11-02 save (Base 1): no eggs yet, two Beegarde on it, 151 cake.
const HATCHERY =
  "Tv/7w/cRStW8l0r31xU3rXMrENbdoQw7vgCMOxxZ2HoAAAAAAAAAAAAAAADttB9BAAAgQQIAAAAAAAAAAAAAAAAAAAABAAAA7peC4C1DTqaZr45qMgdUAQAAAAAAAAAAAAAAAAEAAAAqGPXYSSZG9bqhV5PwkbCfzs3sENlnOJOCC6wDujHIfAAAAAA=";
// Its breeding pair's work entry (EPalWorkableType::OnlyJoinInvisible).
const HATCHERY_WORK =
  "oqDgTj/o26+dnrLMthYwxAAAAAAAgEZAAAAAAAAAAAAUCkX3//9IQAAAAAAAAAAAAAAAAAAAAIAAAAAAAAAAAAAAAAAAAPA/AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAADAYkAAAAAAAABkQAAAAAAAwEdA24WSC90MbEC/WzfPwTTonKZgJ8YSHvGqcysQ1t2hDDu+AIw7HFnYek7/+8P3EUrVvJdK99cVN60BAAAAAAAnAAAATXVsdGlFbGVjdHJpY0hhdGNoaW5nUGFsRWdnV2l0aEJyZWVkXzAAAAIAAAAAAAAAAAEAAAAAAAAAAnMrENbdoQw7vgCMOxxZ2HoAAAAAAAAAAA==";

describe("the Ancient Hatchery", () => {
  it("names its cake box from the tail of its bytes", () => {
    const data = concreteModelBytes(reader, b64(HATCHERY), "MultiElectricHatchingPalEggWithBreed");
    expect([data["concrete_model_type"], data["cake_container_id"]]).toEqual(["PalMapObjectMultiHatchingEggWithBreedModel", "10eccdce-9338-67d9-03ac-0b827cc831ba"]);
  });

  it("reads its breeding pair's work entry", () => {
    const data = workBytes(reader, b64(HATCHERY_WORK), "EPalWorkableType::OnlyJoinInvisible");
    expect([data["id"], data["assign_define_data_id"], data["owner_map_object_model_id"]]).toEqual([
      "4ee0a0a2-afdb-e83f-ccb2-9e9dc43016b6",
      "MultiElectricHatchingPalEggWithBreed_0",
      "d6102b73-3b0c-a1dd-3b8c-00be7ad8591c",
    ]);
  });
});

// An Ancient Farm on the 2026-11-03 save (Base 4): wheat, 225 of its 270 seconds grown, watered.
const ANCIENT_FARM = "MaGc7LX9Qme4lsrKtXath7oxuuw/u0qsjWghfR1ThNQAAAAACAAAAFdoZWF0X1MAA2YwVT8AAIA/AACHQwzZYEMAAAAAAAAAAA==";

describe("the Ancient Farm", () => {
  it("reads its crop like a farm plot", () => {
    const data = concreteModelBytes(reader, b64(ANCIENT_FARM), "AncientFarmBlock");
    expect([data["concrete_model_type"], data["crop_data_id"], data["current_state"], data["crop_progress_rate_value"], data["water_stack_rate_value"]]).toEqual([
      "PalMapObjectFarmBlockV2Model",
      "Wheat_S",
      3,
      expect.closeTo(0.833, 3),
      1,
    ]);
  });
});
