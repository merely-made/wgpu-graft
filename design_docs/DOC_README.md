# Wgpu-graft documentation

This is the canonical index for active documentation in this repository.

## Working principles

- Graft owns native resource import into a host-owned `wgpu` device. Producer
  policy, browser navigation policy, profile policy, and app embedding remain
  in the producer or host crates.
- Treat resource ownership as part of the ABI. Copyable descriptors may carry
  only borrowed handles or scalar metadata; descriptors whose handles are
  consumed by a driver need move-only ownership in the safe API.
- Keep compile coverage, resource-import shape, pixel correctness, and headed
  hardware receipts as separate claims.
- Keep the feature-selected wgpu row identical to the host row and re-export
  the selected `wgpu` / `wgpu_hal` pair at the public boundary.
- Publish and tag the exact source being described. A public API change after
  publication receives a new crate version rather than reusing the old one.
- For cross-repo release work, record the plan here and cite sibling repos by
  path rather than copying their local plans.

## Active documents

- [Documentation policy](DOC_POLICY.md): shared documentation rules and the
  Wgpu-graft local addendum.
- [Wgpu triplet release plan](2026-09-03_wgpu_triplet_release_plan.md):
  release-gating plan for Graft, Scry, and Weld after the browser-surface
  architecture review.
## Legacy reference documents

These predate this `design_docs/` root and still live under `docs/`. Treat them
as active reference material until a deliberate doc-hygiene pass moves or
archives them.

- `docs/testing.md`: runtime validation and demo receipts.
- `docs/project_wgpu_graft.md`: project context and platform-path summary.
- `docs/2026-05-07_slint_upstream_sync_plan.md`: historical Slint upstream sync
  plan.
- `docs/2026-05-27_metal_objc2_wgpu29_arm.md`: Metal/objc2 compatibility note.
- `docs/2026-05-27_xilem_zero_copy_seam.md`: Xilem zero-copy seam note.
- `docs/2026-06-02_bevy_gpui_zero_copy_plan.md`: Bevy/GPUI zero-copy plan.

## Maintainer-owned description

`design_docs/PROJECT_DESCRIPTION.md` is reserved for the maintainer and has not
yet been created. The root [README](../README.md) and
`docs/project_wgpu_graft.md` remain the public project description until the
maintainer supplies it.
