# Renderer Major Update

## Description

In the current design, many flaws has appeared:

- `Material`, `Texture`, `Shader` are GPU side resources. They should be handled by the renderer. Currently assets manager owns them.

- `MaterialMetadata`, `MaterialTemplateMetadata`, and `TextureMetadata` are the elements that should be treated as assets

- Altough `Material` propsed generic API to be created, it is not flexible. We cannot define materials like `ColorMaterial`.

## Proposed improvements plan

To address the limitations above we are going to restructure the rendering flow.
This would have an impact on `nova-core`, and `nova-2d`.

### Nova Core

At this level, we will redefine what should be an asset and what should be handled by the renderer.
The migration would first convert 

- `Material` into `GpuMaterial`
- `Texture` into `GpuTexture`

These elements are owned by the renderer. The user would never need to create them.

`TextureMetadata` will be simply called `Texture`. This is the asset a user can create. The user can define what is the source of a texture, the texture configuration, and the sampler configuration.
The rendering engine should convert these into a gpu resource.

`Sampler` would be managed by the renderer as well. As we said we will expose the configuration as part of the texture configuration.

#### Texture asset design

- `Texture` asset will provide api to create it from file or raw bytes. Right now we default for Srgba8.
- At render phase we resolve the texture asset data, create a `GpuTexture` if not cached. Reuse the cached texture.

- Point to discuss: `Texture` keeps always the raw data or throw it away when uplaoding it into gpu?

#### Resturcture of material system

a `Material` trait will be defined with the following interface

- `shader()` this function returns a shader source
- `template()` this function will return `MaterialTemplateDescriptor`
- `compile()` this function will produce a `GpuMaterial` by setting all the uniforms and bounded textures.

The concrete type that implements `Material` trait will be responsible off setting the correct template layout (shader + template).
`MaterialTemplateDescriptor` role stay unchanged. It defines the data layout of the shader. vertex shader and fragment shader are no longer part of it's definition. They will be moved into `MaterialTemplate`.

#### Impact on Assets Manager

- `Asset` trait definition will no longer need a `Metadata` associated type.
Adding an new asset will be either by inserting directly with an insert/add function, or from a file (currently won't be used).
- `AssetsLoader` will be removed. Assets constructing will be done directly by the user, or from a file that describe the assets (feature will be implemented when serialization system is put in place. We simply call load_from_file to get the data).
- Assets resolution will be removed. We won't need it.
- Assets system must provide a way to query an asset not just by the concrete type, but also with a trait implementation. (More explanation on `[DrawBatch]` section)


All the current assets will not be considered as one except for the definition of `Texture` as it holds the cpu side of the data.


#### DrawBatch impact

The overall functionnality did not change.
`DrawBatch` will be use builder pattern to be constructed. It should be immutable.
vertices/indices are created on `new` call. we can set instances with builder pattern.
`Handle<Material>` won't be accepted anymore. To get a handle to a material we need to use `GenericHandle` (Other proposition is welcomed. We need a way to tell the assets manager we want an asset that is a material).

#### Rendering flow

Rendering flow should follow the next steps. It didn't change that much but some steps need more refinements

- an iterator of `DrawBatch` is submitted (as before).
- Upload geometry and instances to `geometry pool` or `staging buffer` depending on wether they are shared or dynamic (as before).
- Draw call preperations. For each draw batch:
    - we resolve the shader and template from the implementation of `Material`.
    - create the pipeline as before from the template.
    - caching the compiled pipeline to retrieve it next frames.
    - compiling the material into gpu material and create the bind group.
    - material bind groups creation will be done each frame. in this case material data may change, we need to reupload it as we do with scene bind group.
    - set up the buffers to the render pass and issue a draw call.

Caching strategy must be set in place. Elements like samplers, textures, shaders, and pipelines


## Nova 2d

The rendering architecture will change completely.

The new abstraction will give us the possibility to have specialized material types.
We will define materials for 2d environnement.

- As a first step we will support only sprites rendering. Quads will be removed.
- The renderer will provide api that takes the sprite, a transform and a material.
- These elements will be forwarded to the batcher.
- As before the batcher groups elements by z-index and material.
- Transform will be used to build the instance data.
- Overall behaviour did not change, only how we expose it.

## Goal

- Analyze the requirements
- Analyze the code structure.
- From the analysis result, construct a solid implementation plan. After validation go to execution.

## Success mark

Being able to render a normal sprite (colored quad first and textured one) with custom materials using the `Material` trait, defining shader and template.

## Notes

If any ambiguity is detected feel free to ask for it. Use the next section to put your questions if any.

## Questions

