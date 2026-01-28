---
description: Create or evaluate UI/UX designs and wireframes
---

# Wireframe & Design Helper

Use this workflow before implementing UI changes to visualize the layout or evaluate current designs.

## Steps

1. **Clarify Requirements**
   - What page or component are we designing? (e.g., "Download Dashboard", "Settings Modal")
   - What are the key user actions?
   - Any specific style references?

2. **Generate Wireframe/Mockup**
   Use the `generate_image` tool to create a visual representation.

   _Example Prompt:_

   > "Generate a high-fidelity wireframe for a modern file download dashboard. Dark mode, glassmorphism style. Features: active download list with progress bars, speed graph, and a sidebar for navigation."

3. **Evaluate & Iterate**
   - Review the generated image.
   - Refine the prompt if needed to adjust layout, colors, or components.

4. **Implementation Guide**
   Once the design is approved, analyze the image and current codebase (`src/flasharr/static/css`, `templates/`) to plan the implementation.
