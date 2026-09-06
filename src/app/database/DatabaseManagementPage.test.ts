import {describe,expect,it} from "vitest";
import {formatBytes} from "./DatabaseManagementPage";

describe("database management formatting",()=>{
 it("formats database sizes for compact cards",()=>{
  expect(formatBytes(0)).toBe("0 B");
  expect(formatBytes(1024)).toBe("1.00 KB");
  expect(formatBytes(1967370240)).toBe("1.83 GB");
 });
});