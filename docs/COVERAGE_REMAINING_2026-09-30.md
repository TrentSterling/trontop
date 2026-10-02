# Remaining production coverage: alpha.51, 2026-09-30

Frozen-source run: `target/coverage/runs/20260930-103155-387/`.
**24,354 / 26,093 production unique lines covered (93.34%)**;
**1,739 missed across 92 files**, zero mixed lines. Ordinary suite, 24 exact
read-only probes and ten analyzer checks passed. No production exclusions.
Work is paused; [resume anchor](RESUME_ANCHOR.md) gives scope and permissions.

This complete inventory is generated from `production-lines.json`.
Ranges refer only to that measured source fingerprint
`6919A9395DB334DB3C597584F80A6829F303167E6AA340FA3AC029841B84487F`.
Any future source edit can shift lines. Inspect the production behavior before
choosing a meaningful test. Line coverage is not branch or acceptance coverage;
native UI, destructive commands and real Startup/service checks have separate
gates. Do not run blanket ignored tests or use mouse automation.

| File | Covered / executable | Missed | Exact uncovered production lines |
| --- | ---: | ---: | --- |
| [src/app.rs](../src/app.rs) | 2538 / 2688 | 150 | 276-282, 284-299, 436, 634, 698, 707-710, 713-714, 736-740, 742, 922-923, 957-958, 963-964, 1007-1008, 1054, 1056, 1068, 1107, 1111, 1163, 1192, 1199, 1255, 1268-1283, 1285-1286, 1289-1290, 1292-1299, 1601, 1608, 1623, 1664, 1670, 1683-1684, 1705, 1723-1724, 1730, 1739-1740, 1745, 1757-1758, 1767-1768, 1777-1778, 1788-1789, 1803-1806, 1886-1887, 1899-1900, 1921-1922, 1967-1968, 1992-1994, 2009-2010, 2032-2033, 2223, 2256, 2273, 2278, 2444, 2449, 2487, 2501, 2645, 2772-2773, 2835-2836, 2840-2841, 3115, 3180-3182, 3219, 3246-3247, 3451, 3517, 3519-3520 |
| [src/tray.rs](../src/tray.rs) | 55 / 116 | 61 | 32-34, 36, 61, 137-139, 142-150, 152-158, 160-164, 173-177, 179-189, 191-192, 194-204, 208-210 |
| [src/export/native.rs](../src/export/native.rs) | 0 / 57 | 57 | 12-14, 17, 19-21, 23, 25-30, 36-61, 63-73, 75-79, 81 |
| [src/startup/control/native.rs](../src/startup/control/native.rs) | 305 / 357 | 52 | 19-23, 26-29, 31, 34, 70-71, 75, 119, 128, 155-156, 159, 167, 183, 190, 201, 219, 241, 305-306, 334-342, 344, 347, 362-363, 383-385, 459-461, 464-466, 471-473 |
| [src/specs/native/wmi.rs](../src/specs/native/wmi.rs) | 202 / 251 | 49 | 37-39, 65-66, 74, 173-176, 204, 280-283, 291-294, 301, 307, 336, 347, 382, 390, 393-399, 408, 414, 422, 428, 433, 438, 452-454, 456-463 |
| [src/specs/os/native.rs](../src/specs/os/native.rs) | 267 / 314 | 47 | 20, 60, 69, 87, 97-98, 175, 195-196, 204-205, 214, 216, 218-219, 229, 231-232, 248, 250, 252-258, 288, 308-309, 311, 317, 327-332, 349, 365-366, 383, 388, 405, 426, 449, 452 |
| [src/windows_metrics.rs](../src/windows_metrics.rs) | 214 / 259 | 45 | 77-79, 86-87, 99-100, 114-115, 130-131, 134, 149-155, 159-165, 171-180, 202, 216-219, 244, 272, 275, 294 |
| [src/specs/native/mod.rs](../src/specs/native/mod.rs) | 116 / 159 | 43 | 42-43, 45, 47, 63, 84-86, 142-143, 147, 152, 177-178, 188, 190-192, 195-219 |
| [src/main.rs](../src/main.rs) | 20 / 60 | 40 | 39-41, 43-47, 53-55, 61-72, 75-78, 82, 85-88, 90, 100-101, 103, 112-115 |
| [src/sampler.rs](../src/sampler.rs) | 403 / 439 | 36 | 120, 135, 179-181, 183-188, 238, 243, 270-271, 273, 308, 312, 333, 343, 346, 455, 459, 467, 474-475, 479-480, 482, 515-518, 520, 522-523 |
| [src/preferences.rs](../src/preferences.rs) | 174 / 209 | 35 | 66-80, 96, 188, 204-211, 237-238, 249-256 |
| [src/app/graphs.rs](../src/app/graphs.rs) | 818 / 852 | 34 | 112, 273-274, 301-302, 347-350, 362-365, 415, 485, 555-556, 558-565, 581-583, 602, 664, 856, 968, 974, 1065 |
| [src/app/overview.rs](../src/app/overview.rs) | 1009 / 1041 | 32 | 88, 252, 295-296, 309, 376, 380-381, 383, 465-469, 593-597, 810, 868, 1086, 1108-1110, 1122, 1165, 1239, 1343, 1361-1362, 1397 |
| [src/app/diagnostics.rs](../src/app/diagnostics.rs) | 237 / 268 | 31 | 86, 89, 91, 105, 151-157, 159-167, 169-170, 189-191, 193, 195, 210-211, 218, 334 |
| [src/service_control/native.rs](../src/service_control/native.rs) | 42 / 73 | 31 | 14-17, 19-29, 31-32, 51-58, 68-71, 103, 107 |
| [src/specs/devices.rs](../src/specs/devices.rs) | 322 / 353 | 31 | 60-62, 99, 104-106, 132, 142, 162, 164-169, 182, 186, 214, 253, 261, 277, 301, 315-319, 389, 442, 468 |
| [src/specs/cpu.rs](../src/specs/cpu.rs) | 656 / 686 | 30 | 298-311, 331-333, 560-568, 772, 894, 922, 936 |
| [src/windows_metrics/startup.rs](../src/windows_metrics/startup.rs) | 162 / 191 | 29 | 58-59, 76, 78-80, 82, 92-93, 122, 133-134, 138, 198-200, 202, 236-237, 243, 249, 252, 255-256, 259-260, 279, 282, 284 |
| [src/specs/storage/native.rs](../src/specs/storage/native.rs) | 141 / 169 | 28 | 128, 141-143, 154, 164, 172, 182, 186-187, 199-201, 217-219, 221, 223, 225-229, 233, 235, 237, 239-240 |
| [src/platform/tree.rs](../src/platform/tree.rs) | 78 / 105 | 27 | 23-25, 27, 29-33, 43, 72, 77, 79-80, 89-93, 111-112, 143-146, 149-150 |
| [src/service_control.rs](../src/service_control.rs) | 238 / 265 | 27 | 38, 41-43, 181, 183-185, 301, 304, 311, 326, 350, 379-392 |
| [src/theme_studio.rs](../src/theme_studio.rs) | 699 / 726 | 27 | 80-81, 94, 183, 186-187, 229-230, 258-259, 349-352, 363-364, 367, 385-386, 403, 526, 554, 558, 603, 652, 673, 798 |
| [src/specs/graphics.rs](../src/specs/graphics.rs) | 247 / 273 | 26 | 70, 73, 75-81, 83, 113, 152, 154, 219, 243-246, 325, 329-331, 351, 374-376 |
| [src/storage_sensors/mod.rs](../src/storage_sensors/mod.rs) | 308 / 334 | 26 | 36, 38-41, 162, 177-180, 217, 311, 346, 395-396, 403-404, 485-486, 491, 495, 514-518 |
| [src/app/gpus.rs](../src/app/gpus.rs) | 121 / 146 | 25 | 71, 87-95, 108, 110-113, 115-118, 145-148, 173, 175 |
| [src/gpu_adapters/native.rs](../src/gpu_adapters/native.rs) | 198 / 223 | 25 | 27, 41, 70-73, 78, 113, 117, 139-140, 147, 157, 167, 183, 204, 217, 220, 223, 237, 240, 248, 263, 281, 286 |
| [src/platform.rs](../src/platform.rs) | 139 / 164 | 25 | 86, 98-99, 149-150, 157-158, 188, 191-192, 194, 255-261, 269-275 |
| [src/export.rs](../src/export.rs) | 71 / 95 | 24 | 33-34, 121, 125-135, 138-139, 141-142, 149, 193-196, 199 |
| [src/specs/bridge.rs](../src/specs/bridge.rs) | 235 / 257 | 22 | 53-55, 71-80, 92, 154, 167-168, 170-172, 177, 183 |
| [src/specs/worker.rs](../src/specs/worker.rs) | 241 / 263 | 22 | 166-168, 218-221, 234-235, 241-245, 265, 311, 320, 325, 335, 386, 392, 402 |
| [src/app/system.rs](../src/app/system.rs) | 776 / 797 | 21 | 87-89, 157-160, 173, 234, 255-257, 295, 297, 336-337, 376, 776, 896, 918-919 |
| [src/specs/graphics/native.rs](../src/specs/graphics/native.rs) | 218 / 239 | 21 | 36-38, 92, 98, 125-127, 129-130, 176, 179, 196, 199, 208-210, 255-257, 275 |
| [src/app/disks.rs](../src/app/disks.rs) | 141 / 161 | 20 | 44, 51-57, 59-64, 66-70, 185 |
| [src/app/sensors.rs](../src/app/sensors.rs) | 501 / 521 | 20 | 109, 173, 183, 531, 611, 657-671 |
| [src/specs/native/setupapi.rs](../src/specs/native/setupapi.rs) | 243 / 263 | 20 | 100, 125, 131-138, 145, 148, 263, 300, 325, 328, 367, 384, 424, 434 |
| [src/app/inventory.rs](../src/app/inventory.rs) | 290 / 309 | 19 | 57-59, 143, 177-178, 182, 221, 226, 285, 294-298, 308, 315, 397, 445 |
| [src/diagnostics.rs](../src/diagnostics.rs) | 115 / 134 | 19 | 99-106, 108, 110-111, 113, 116, 119, 122, 125, 235, 263, 266 |
| [src/inventory.rs](../src/inventory.rs) | 180 / 199 | 19 | 67, 116, 158-160, 200, 214-219, 239-243, 245, 247 |
| [src/specs/cpu/native.rs](../src/specs/cpu/native.rs) | 119 / 138 | 19 | 31, 34-37, 45, 56, 94, 99, 101, 115, 123, 138, 159-162, 170, 173 |
| [src/failure.rs](../src/failure.rs) | 195 / 213 | 18 | 37, 64-73, 143, 215, 254, 258, 265, 283, 309 |
| [src/gpu_sensors/nvml.rs](../src/gpu_sensors/nvml.rs) | 141 / 158 | 17 | 123, 126, 137-141, 143-147, 154, 245, 256-258 |
| [src/disk_activity/native.rs](../src/disk_activity/native.rs) | 124 / 140 | 16 | 28, 34, 37, 40, 62, 65-66, 77, 91, 104, 115, 118, 121, 136, 139, 143 |
| [src/specs/storage/management.rs](../src/specs/storage/management.rs) | 145 / 161 | 16 | 15, 28-33, 83, 93, 96, 104, 107, 113, 116, 152, 159 |
| [src/specs/board.rs](../src/specs/board.rs) | 307 / 322 | 15 | 47, 58, 60, 116, 378-380, 384-387, 407-410 |
| [src/tray/native/worker.rs](../src/tray/native/worker.rs) | 126 / 141 | 15 | 82-89, 116, 152, 249-253 |
| [src/app/storage.rs](../src/app/storage.rs) | 230 / 243 | 13 | 20-27, 35, 61, 76, 179, 224 |
| [src/cpu_clock.rs](../src/cpu_clock.rs) | 200 / 213 | 13 | 47, 51-52, 212, 296, 320-324, 327, 330, 340 |
| [src/specs/bridge/native.rs](../src/specs/bridge/native.rs) | 90 / 103 | 13 | 21, 23-30, 32-33, 96, 108 |
| [src/specs/network.rs](../src/specs/network.rs) | 238 / 251 | 13 | 221-226, 238, 279, 380, 394, 397, 409, 414 |
| [src/storage_sensors/native.rs](../src/storage_sensors/native.rs) | 145 / 158 | 13 | 61, 108, 125, 155, 175, 194, 213, 216, 221, 224-227 |
| [src/process_cpu.rs](../src/process_cpu.rs) | 110 / 122 | 12 | 78, 186-191, 194-197, 207 |
| [src/startup/control/worker.rs](../src/startup/control/worker.rs) | 72 / 84 | 12 | 24, 27, 34, 63, 94, 99-105 |
| [src/widgets.rs](../src/widgets.rs) | 1720 / 1732 | 12 | 460-461, 463, 465, 770, 782-783, 1506-1507, 1563, 1599, 1614 |
| [src/disk_activity.rs](../src/disk_activity.rs) | 235 / 246 | 11 | 355-365 |
| [src/specs/native/registry.rs](../src/specs/native/registry.rs) | 95 / 106 | 11 | 30, 38, 57, 145, 177-181, 184, 191 |
| [src/specs/network/native.rs](../src/specs/network/native.rs) | 162 / 173 | 11 | 52-54, 62, 103, 140, 143, 146, 153, 192, 229 |
| [src/preferences/file.rs](../src/preferences/file.rs) | 142 / 152 | 10 | 40, 42, 44, 52, 64, 73, 107, 114, 152, 198 |
| [src/app/service_controls.rs](../src/app/service_controls.rs) | 267 / 276 | 9 | 183-185, 247-248, 287, 294, 306, 322 |
| [src/process_icons/mod.rs](../src/process_icons/mod.rs) | 169 / 178 | 9 | 64, 67, 74, 90, 94, 106, 135-136, 162 |
| [src/specs/os.rs](../src/specs/os.rs) | 35 / 44 | 9 | 49, 51-55, 63-64, 66 |
| [src/theme.rs](../src/theme.rs) | 374 / 383 | 9 | 322-323, 325, 327-328, 427-429, 476 |
| [src/theme/gradient.rs](../src/theme/gradient.rs) | 117 / 126 | 9 | 45, 62, 84-88, 111, 139 |
| [src/app/graphs/wall.rs](../src/app/graphs/wall.rs) | 316 / 324 | 8 | 82, 261-265, 268, 463 |
| [src/app/process_controls.rs](../src/app/process_controls.rs) | 153 / 161 | 8 | 26, 29, 32-36, 71 |
| [src/process_actions/tree.rs](../src/process_actions/tree.rs) | 103 / 111 | 8 | 81, 85, 110-111, 142, 148-150 |
| [src/specs/native/smbios.rs](../src/specs/native/smbios.rs) | 155 / 163 | 8 | 97, 100, 106, 111, 113, 128, 161-162 |
| [src/app/startup_controls.rs](../src/app/startup_controls.rs) | 136 / 143 | 7 | 71-74, 77, 114, 153 |
| [src/export/file.rs](../src/export/file.rs) | 65 / 72 | 7 | 76-77, 81-84, 97 |
| [src/platform/suspension.rs](../src/platform/suspension.rs) | 95 / 102 | 7 | 32-35, 77-79 |
| [src/specs/live.rs](../src/specs/live.rs) | 227 / 234 | 7 | 307, 309-313, 389 |
| [src/app/graphs/history.rs](../src/app/graphs/history.rs) | 602 / 608 | 6 | 158, 166, 189-190, 732, 777 |
| [src/gpu_sensors.rs](../src/gpu_sensors.rs) | 66 / 72 | 6 | 95-100 |
| [src/process_actions.rs](../src/process_actions.rs) | 185 / 191 | 6 | 132, 139-140, 213, 217, 248 |
| [src/specs/devices/native.rs](../src/specs/devices/native.rs) | 108 / 114 | 6 | 106, 111-112, 129, 139, 155 |
| [src/theme/magic.rs](../src/theme/magic.rs) | 72 / 78 | 6 | 31-36 |
| [src/app/preferences.rs](../src/app/preferences.rs) | 113 / 118 | 5 | 36, 71-73, 94 |
| [src/process_icons/native.rs](../src/process_icons/native.rs) | 136 / 141 | 5 | 97, 122, 127, 156, 170 |
| [src/startup/control/observations.rs](../src/startup/control/observations.rs) | 147 / 152 | 5 | 56, 72, 79, 89, 162 |
| [src/app/networks.rs](../src/app/networks.rs) | 110 / 114 | 4 | 8-9, 102, 110 |
| [src/startup.rs](../src/startup.rs) | 152 / 156 | 4 | 67-69, 117 |
| [src/gpu_adapters.rs](../src/gpu_adapters.rs) | 88 / 91 | 3 | 130, 133, 136 |
| [src/specs/memory.rs](../src/specs/memory.rs) | 400 / 403 | 3 | 530-532 |
| [src/specs/report.rs](../src/specs/report.rs) | 240 / 243 | 3 | 172, 235, 241 |
| [src/startup/control.rs](../src/startup/control.rs) | 119 / 122 | 3 | 43, 83, 146 |
| [src/app/export.rs](../src/app/export.rs) | 82 / 84 | 2 | 15, 74 |
| [src/app/window.rs](../src/app/window.rs) | 51 / 53 | 2 | 24, 32 |
| [src/export/encode.rs](../src/export/encode.rs) | 329 / 331 | 2 | 120, 171 |
| [src/model.rs](../src/model.rs) | 130 / 132 | 2 | 40, 148 |
| [src/network_identity.rs](../src/network_identity.rs) | 39 / 41 | 2 | 29, 50 |
| [src/memory_metrics.rs](../src/memory_metrics.rs) | 44 / 45 | 1 | 81 |
| [src/theme/storage.rs](../src/theme/storage.rs) | 119 / 120 | 1 | 40 |
| [src/theme/typography.rs](../src/theme/typography.rs) | 43 / 44 | 1 | 19 |

Total missed: **1,739**. Complete LLVM JSON/LCOV and HTML remain in the run folder.
