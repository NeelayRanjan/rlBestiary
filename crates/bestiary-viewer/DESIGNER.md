# Creature designer

Open an existing world with `bestiary-viewer WORLD.db --panel designer`, or choose
**Creature designer** in the sidebar. A saved world supports editing and combines;
start its daemon to enable release.

1. Load a zoo preset or copy the inspected creature. The draft is separate from
   the world. Choose a bone gene under **Bones and recursion**, then add an
   attached bone, change dimensions, shape, density, color, or recursion count.
2. Under **Joints and attachment points**, connect existing bone genes, choose
   the joint type and angle limits, set subtree scale, or toggle reflection and
   terminal recursion. A self-joint builds a repeated segment.
3. Drag the **Jp** and **Jc** handles on the 3D body to move the parent and child
   attachments along their surfaces. The numeric face controls provide exact
   coordinates. Right-drag orbits the camera; the wheel zooms.
4. Shift-click two bone surfaces to draw a muscle. Alternatively, choose two
   expanded part numbers and **Connect muscle**. Endpoints must be reachable
   within the world's maximum joint span. **Mo** and **Mi** move the selected
   muscle's endpoints; **Muscles** controls cross-section and material genes.
   Repeated and reflected instances share their underlying bone gene.
5. Attach sensors and set life-history genes. Read **Validity** for expanded
   part count, mass, metabolic estimates, invalid values and clipped expansion.
   The existing genesis guarantee supplies any missing joint actuation.
6. **Run combine** measures the nine physics trials in a background worker and
   displays the trait vector and measured Sprint replay. **Live muscle test**
   displays current activation; recorded replays contain poses only. Editing
   invalidates old measurements. Recorded Sprint preview supports camera clips.
7. With a live daemon, choose a population count and world X/Z location, then
   **Release into running world**. Release requires a current viable combine
   with a clear energy audit. The daemon transfers resources from the world and
   reports any population-cap or resource error.
8. **Save genome** and **Load genome** use the path field. **Copy share string**
   copies the exact current genome to the system clipboard; paste a string into
   the text box and choose **Load share string** to import it.

The eight presets' documented first-generation fixture releases a pair at
`[51.2, 19.2]` metres into a fresh seed-42 demo world with 24 resident grazers.
Their authored behavior and life-history knobs are in `config/zoo.ron`; survival
at other locations, population sizes, or parameter settings is not guaranteed.
