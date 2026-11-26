# Progress

| Milestone | State |
|---|---|
| M0 Foundations | Workspace, rules, checks and CI are in. The tests pass and frames render on macOS. On the Intel Mac typing at 10,000 blocks is inside its budget (2.9 ms a frame) and opening was not (0.77 s against 150 ms, measured as a Markdown import on the main thread). Opening saved bytes now reads in the background and takes one frame of the drawing thread; on the Linux machine it is 185 ms to the first frame, still over 150 ms, and not yet measured on the Mac. The view builds and passes its tests on the optional faster backend as well as the default one. The cross-repository requests are written in `docs/requests.md` and not filed. |
| M1 Core editing | Headless half done: tier 1 blocks, marks, cross-block selection, undo, input rules, JSON and Markdown, limits. View half started: drawing, selection, keyboard, pointer, text input, clipboard, scrolling and drawing only the rows in view work in headless windows and render on Linux and macOS; not yet used by hand. |
| M2 Block UX | Started: only the rows in view are drawn, the insert menu works, and find steps through matches. Handles, the block menu, the format toolbar, links, callout, toggle, drag and drop and replace are not built. |
| M3 Rich blocks | Not started. |
| M4 Extension API | Not started. |
| M5 Agents and collaboration | Not started. Sync by version and anchors exist in the store. |
| M6 Hardening | Not started. |
