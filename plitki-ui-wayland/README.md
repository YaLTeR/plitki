Points of interest:
- rendering only on frame callbacks,
- rendering in a separate thread as to not block input,
- predicting the presentation time and rendering for that presentation time so
  that even at low FPS there's no visual delay.
