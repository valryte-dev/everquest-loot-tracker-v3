import {describe,expect,it} from "vitest";
import {
 PAPERDOLL_LEFT_SLOTS,PAPERDOLL_RIGHT_SLOTS,PAPERDOLL_SLOT_GROUPS,
 PAPERDOLL_TOP_SLOTS,PAPERDOLL_WEAPON_SLOTS,equipmentSlotKey,itemIconPath,
} from "./EquipmentPaperDoll";

describe("equipment paper doll",()=>{
 it("normalizes EverQuest left/right equipment labels",()=>{
  expect(equipmentSlotKey("Left Ear")).toBe("left-ear");
  expect(equipmentSlotKey("Ear2")).toBe("right-ear");
  expect(equipmentSlotKey("Wrist 1")).toBe("left-wrist");
  expect(equipmentSlotKey("Right Finger")).toBe("right-finger");
  expect(equipmentSlotKey("Shoulder")).toBe("shoulders");
  expect(equipmentSlotKey("Ear",0)).toBe("left-ear");
  expect(equipmentSlotKey("Ear",1)).toBe("right-ear");
  expect(equipmentSlotKey("Wrist",0)).toBe("left-wrist");
  expect(equipmentSlotKey("Wrist",1)).toBe("right-wrist");
  expect(equipmentSlotKey("Fingers",0)).toBe("left-finger");
  expect(equipmentSlotKey("Fingers",1)).toBe("right-finger");
 });

 it("wraps the model with the P99 Planner paper-doll groups",()=>{
  expect(PAPERDOLL_TOP_SLOTS.map(slot=>slot.key)).toEqual(["neck","face","head"]);
  expect(PAPERDOLL_LEFT_SLOTS).toHaveLength(8);
  expect(PAPERDOLL_RIGHT_SLOTS).toHaveLength(8);
  expect(PAPERDOLL_WEAPON_SLOTS.map(slot=>slot.key)).toEqual(["primary","secondary"]);
  expect(PAPERDOLL_SLOT_GROUPS.flat()).toHaveLength(21);
 });

 it("uses the bundled icon atlas only for known icon ids",()=>{
  expect(itemIconPath(601)).toBe("/item-icons/Item_601.png");
  expect(itemIconPath()).toBeUndefined();
 });
});
