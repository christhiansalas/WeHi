//! Compositor GPU del logo + fondo usando wgpu.
//!
//! El contexto (device, queue, pipeline) se inicializa UNA vez por
//! proceso vía `OnceLock`. Si la inicialización falla (CI headless,
//! sandbox sin adapter), `shared()` devuelve `None` y `compose_logo_auto`
//! cae al camino CPU. Una vez el contexto existe, se comparte entre
//! hilos de rayon — wgpu garantiza Send+Sync en Device y Queue.

use std::sync::OnceLock;

use bytemuck::{Pod, Zeroable};
use image::{DynamicImage, RgbaImage};
use pollster::FutureExt;
use wgpu::util::DeviceExt;

use crate::config::LogoConfig;
use crate::error::ImgError;
use crate::overlay::{compose_logo, LoadedLogo};

const SHADER: &str = r#"
// El orden coloca bg_color (vec4, alineamiento 16) primero para evitar
// padding interno y garantizar que el tamaño en CPU (80 bytes) coincida
// con el que WGSL espera tras redondear a múltiplo de 16.
struct Uniforms {
    bg_color: vec4<f32>,
    base_size: vec2<f32>,
    logo_pos: vec2<f32>,
    logo_size: vec2<f32>,
    bg_pos: vec2<f32>,
    bg_size: vec2<f32>,
    opacity: f32,
    has_bg: f32,
    _p0: f32,
    _p1: f32,
    _p2: f32,
    _p3: f32,
};

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var base_tex: texture_2d<f32>;
@group(0) @binding(2) var base_samp: sampler;
@group(0) @binding(3) var logo_tex: texture_2d<f32>;
@group(0) @binding(4) var logo_samp: sampler;

@vertex
fn vs_main(@builtin(vertex_index) vid: u32) -> @builtin(position) vec4<f32> {
    var p = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0), vec2<f32>( 1.0, -1.0), vec2<f32>(-1.0,  1.0),
        vec2<f32>( 1.0, -1.0), vec2<f32>( 1.0,  1.0), vec2<f32>(-1.0,  1.0)
    );
    return vec4<f32>(p[vid], 0.0, 1.0);
}

fn blend(base: vec4<f32>, over: vec4<f32>) -> vec4<f32> {
    let a = over.a;
    let inv = 1.0 - a;
    let out_a = a + base.a * inv;
    return vec4<f32>(over.rgb * a + base.rgb * inv, out_a);
}

@fragment
fn fs_main(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let pix = pos.xy;
    let base_uv = pix / u.base_size;
    var color = textureSample(base_tex, base_samp, base_uv);

    if (u.has_bg > 0.5) {
        if (pix.x >= u.bg_pos.x && pix.x < u.bg_pos.x + u.bg_size.x &&
            pix.y >= u.bg_pos.y && pix.y < u.bg_pos.y + u.bg_size.y) {
            color = blend(color, u.bg_color);
        }
    }

    if (pix.x >= u.logo_pos.x && pix.x < u.logo_pos.x + u.logo_size.x &&
        pix.y >= u.logo_pos.y && pix.y < u.logo_pos.y + u.logo_size.y) {
        let luv = (pix - u.logo_pos) / u.logo_size;
        var l = textureSample(logo_tex, logo_samp, luv);
        l.a = l.a * u.opacity;
        color = blend(color, l);
    }

    return color;
}
"#;

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct Uniforms {
    bg_color: [f32; 4],
    base_size: [f32; 2],
    logo_pos: [f32; 2],
    logo_size: [f32; 2],
    bg_pos: [f32; 2],
    bg_size: [f32; 2],
    opacity: f32,
    has_bg: f32,
    _p0: f32,
    _p1: f32,
    _p2: f32,
    _p3: f32,
}

pub struct GpuOverlay {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    sampler: wgpu::Sampler,
    bgl: wgpu::BindGroupLayout,
}

impl GpuOverlay {
    /// Inicializa el contexto wgpu. Bloquea el hilo hasta resolver el
    /// adapter; suficiente porque solo se llama una vez por proceso.
    pub fn new() -> Result<Self, ImgError> {
        async fn init() -> Result<GpuOverlay, ImgError> {
            let instance = wgpu::Instance::default();
            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::HighPerformance,
                    force_fallback_adapter: false,
                    compatible_surface: None,
                })
                .await
                .ok_or_else(|| ImgError::processing("wgpu: no se encontró adapter"))?;
            let (device, queue) = adapter
                .request_device(
                    &wgpu::DeviceDescriptor {
                        label: Some("wehi_gpu_overlay"),
                        required_features: wgpu::Features::empty(),
                        required_limits: wgpu::Limits::downlevel_defaults(),
                        memory_hints: wgpu::MemoryHints::Performance,
                    },
                    None,
                )
                .await
                .map_err(|e| ImgError::processing(format!("wgpu device: {e}")))?;

            let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("wehi_overlay_shader"),
                source: wgpu::ShaderSource::Wgsl(SHADER.into()),
            });

            let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("wehi_overlay_bgl"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 4,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("wehi_overlay_pl"),
                bind_group_layouts: &[&bgl],
                push_constant_ranges: &[],
            });

            let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("wehi_overlay_rp"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            });

            let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("wehi_overlay_sampler"),
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::FilterMode::Nearest,
                ..Default::default()
            });

            Ok(GpuOverlay {
                device,
                queue,
                pipeline,
                sampler,
                bgl,
            })
        }
        init().block_on()
    }

    /// Compone `logo` sobre `base` según `cfg` enteramente en GPU.
    /// La caja del logo y el padding del fondo se calculan EXACTAMENTE
    /// igual que la versión CPU para garantizar paridad geométrica.
    pub fn compose(
        &self,
        base: &RgbaImage,
        logo: &RgbaImage,
        cfg: &LogoConfig,
    ) -> Result<RgbaImage, ImgError> {
        let (bw, bh) = base.dimensions();
        if bw == 0 || bh == 0 {
            return Err(ImgError::processing("gpu: base con dimensión 0"));
        }
        let (lw, lh) = logo.dimensions();
        if lw == 0 || lh == 0 {
            return Err(ImgError::processing("gpu: logo con dimensión 0"));
        }

        let scale = cfg.scale_pct.clamp(0.001, 1.0);
        let target_w = ((bw as f32 * scale).round() as i64).max(1).min(bw as i64) as u32;
        let target_h = {
            let ratio = lh as f32 / lw as f32;
            ((target_w as f32 * ratio).round() as i64)
                .max(1)
                .min(bh as i64) as u32
        };
        let cx = bw as f32 * cfg.position.x.clamp(0.0, 1.0);
        let cy = bh as f32 * cfg.position.y.clamp(0.0, 1.0);
        let mut x = (cx - target_w as f32 / 2.0).round() as i64;
        let mut y = (cy - target_h as f32 / 2.0).round() as i64;
        if x < 0 {
            x = 0;
        }
        if y < 0 {
            y = 0;
        }
        if x + target_w as i64 > bw as i64 {
            x = bw as i64 - target_w as i64;
        }
        if y + target_h as i64 > bh as i64 {
            y = bh as i64 - target_h as i64;
        }
        let lx = x.max(0) as f32;
        let ly = y.max(0) as f32;

        let (bx, by, bbw, bbh, bgc, has_bg) = if let Some(bg) = cfg.background {
            let pad = (target_w as f32 * bg.padding_pct.max(0.0)).round() as i64;
            let mut bx = lx as i64 - pad;
            let mut by = ly as i64 - pad;
            let mut bbw = target_w as i64 + 2 * pad;
            let mut bbh = target_h as i64 + 2 * pad;
            if bx < 0 {
                bbw += bx;
                bx = 0;
            }
            if by < 0 {
                bbh += by;
                by = 0;
            }
            if bx + bbw > bw as i64 {
                bbw = bw as i64 - bx;
            }
            if by + bbh > bh as i64 {
                bbh = bh as i64 - by;
            }
            let c = bg.color_rgba;
            (
                bx.max(0) as f32,
                by.max(0) as f32,
                bbw.max(0) as f32,
                bbh.max(0) as f32,
                [
                    c[0] as f32 / 255.0,
                    c[1] as f32 / 255.0,
                    c[2] as f32 / 255.0,
                    c[3] as f32 / 255.0,
                ],
                1.0_f32,
            )
        } else {
            (0.0, 0.0, 0.0, 0.0, [0.0; 4], 0.0)
        };

        let uniforms = Uniforms {
            bg_color: bgc,
            base_size: [bw as f32, bh as f32],
            logo_pos: [lx, ly],
            logo_size: [target_w as f32, target_h as f32],
            bg_pos: [bx, by],
            bg_size: [bbw, bbh],
            opacity: cfg.opacity.clamp(0.0, 1.0),
            has_bg,
            _p0: 0.0,
            _p1: 0.0,
            _p2: 0.0,
            _p3: 0.0,
        };

        let base_tex = self.device.create_texture_with_data(
            &self.queue,
            &wgpu::TextureDescriptor {
                label: Some("base"),
                size: wgpu::Extent3d {
                    width: bw,
                    height: bh,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            base.as_raw(),
        );
        let logo_tex = self.device.create_texture_with_data(
            &self.queue,
            &wgpu::TextureDescriptor {
                label: Some("logo"),
                size: wgpu::Extent3d {
                    width: lw,
                    height: lh,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            logo.as_raw(),
        );

        let out_tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("out"),
            size: wgpu::Extent3d {
                width: bw,
                height: bh,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        let ubuf = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("ubuf"),
                contents: bytemuck::bytes_of(&uniforms),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

        let base_view = base_tex.create_view(&Default::default());
        let logo_view = logo_tex.create_view(&Default::default());
        let out_view = out_tex.create_view(&Default::default());

        let bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bind"),
            layout: &self.bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: ubuf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&base_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&logo_view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });

        // bytes_per_row debe estar alineado a 256 para copy_texture_to_buffer.
        let bpr = (bw * 4).next_multiple_of(256);
        let read_buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: (bpr as u64) * (bh as u64),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut enc = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("encoder"),
            });
        {
            let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("rp"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &out_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            rp.set_pipeline(&self.pipeline);
            rp.set_bind_group(0, &bind, &[]);
            rp.draw(0..6, 0..1);
        }
        enc.copy_texture_to_buffer(
            wgpu::ImageCopyTexture {
                texture: &out_tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::ImageCopyBuffer {
                buffer: &read_buf,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(bpr),
                    rows_per_image: Some(bh),
                },
            },
            wgpu::Extent3d {
                width: bw,
                height: bh,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([enc.finish()]);

        let slice = read_buf.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        self.device.poll(wgpu::Maintain::Wait);
        rx.recv()
            .map_err(|e| ImgError::processing(format!("gpu recv: {e}")))?
            .map_err(|e| ImgError::processing(format!("gpu map: {e:?}")))?;

        let data = slice.get_mapped_range();
        let mut out = Vec::with_capacity((bw * bh * 4) as usize);
        let row_bytes = (bw * 4) as usize;
        for row in 0..bh {
            let start = (row * bpr) as usize;
            out.extend_from_slice(&data[start..start + row_bytes]);
        }
        drop(data);
        read_buf.unmap();

        RgbaImage::from_raw(bw, bh, out)
            .ok_or_else(|| ImgError::processing("gpu: buffer de salida inválido"))
    }
}

static SHARED: OnceLock<Option<GpuOverlay>> = OnceLock::new();

/// Devuelve el contexto GPU compartido. `None` si no se pudo inicializar
/// (e.g. CI sin adapter). El llamador debe caer a CPU en ese caso.
pub fn shared() -> Option<&'static GpuOverlay> {
    SHARED
        .get_or_init(|| match GpuOverlay::new() {
            Ok(g) => Some(g),
            Err(e) => {
                eprintln!("wehi: gpu overlay no disponible ({e}), usando CPU");
                None
            }
        })
        .as_ref()
}

/// Compone el logo en GPU. Falla si no hay adapter disponible.
pub fn compose_logo_gpu(
    base: DynamicImage,
    logo: &LoadedLogo,
    cfg: &LogoConfig,
) -> Result<DynamicImage, ImgError> {
    let g = shared().ok_or_else(|| ImgError::processing("gpu no disponible"))?;
    let base_rgba = base.to_rgba8();
    let out = g.compose(&base_rgba, &logo.image, cfg)?;
    Ok(DynamicImage::ImageRgba8(out))
}

/// Intenta GPU; si no hay contexto o falla en runtime, cae a CPU.
/// Es la entrada que usa el pipeline en producción.
pub fn compose_logo_auto(
    base: DynamicImage,
    logo: &LoadedLogo,
    cfg: &LogoConfig,
) -> Result<DynamicImage, ImgError> {
    if let Some(g) = shared() {
        let base_rgba = base.to_rgba8();
        match g.compose(&base_rgba, &logo.image, cfg) {
            Ok(out) => return Ok(DynamicImage::ImageRgba8(out)),
            Err(_) => {
                // Reintenta en CPU con la imagen original.
                return compose_logo(DynamicImage::ImageRgba8(base_rgba), logo, cfg);
            }
        }
    }
    compose_logo(base, logo, cfg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{LogoBackground, LogoPosition};
    use image::{GenericImageView, Rgba};
    use std::path::PathBuf;

    fn imagen_base(w: u32, h: u32, color: [u8; 4]) -> RgbaImage {
        let mut img = RgbaImage::new(w, h);
        for y in 0..h {
            for x in 0..w {
                img.put_pixel(x, y, Rgba(color));
            }
        }
        img
    }

    fn logo_de(w: u32, h: u32, color: [u8; 4]) -> LoadedLogo {
        LoadedLogo {
            image: imagen_base(w, h, color),
            path: PathBuf::from("logo_test.png"),
        }
    }

    /// Si no hay adapter (CI), todos los tests GPU se omiten silenciosamente.
    fn skip_si_no_gpu() -> Option<&'static GpuOverlay> {
        let g = shared();
        if g.is_none() {
            eprintln!("(skipped: no hay adapter wgpu disponible)");
        }
        g
    }

    #[test]
    fn gpu_compose_logo_centrado_es_aproximadamente_el_color_del_logo() {
        let Some(g) = skip_si_no_gpu() else {
            return;
        };
        let base = imagen_base(100, 100, [255, 255, 255, 255]);
        let logo = logo_de(20, 20, [255, 0, 0, 255]);
        let cfg = LogoConfig {
            logo_id: "x".into(),
            position: LogoPosition { x: 0.5, y: 0.5 },
            scale_pct: 0.2,
            opacity: 1.0,
            background: None,
        };
        let out = g.compose(&base, &logo.image, &cfg).unwrap();
        let c = out.get_pixel(50, 50).0;
        // Bilinear vs Lanczos puede mover algunos bits, pero el centro es rojo.
        assert!(
            c[0] > 240 && c[1] < 30 && c[2] < 30,
            "centro debería ser rojo, fue {c:?}"
        );
        let e = out.get_pixel(2, 2).0;
        assert_eq!(e, [255, 255, 255, 255], "esquina sigue blanca");
    }

    #[test]
    fn gpu_compose_logo_con_fondo_pinta_alrededor() {
        let Some(g) = skip_si_no_gpu() else {
            return;
        };
        let base = imagen_base(100, 100, [255, 255, 255, 255]);
        let logo = logo_de(20, 20, [255, 0, 0, 255]);
        let cfg = LogoConfig {
            logo_id: "x".into(),
            position: LogoPosition { x: 0.5, y: 0.5 },
            scale_pct: 0.2,
            opacity: 1.0,
            background: Some(LogoBackground {
                color_rgba: [0, 0, 0, 255],
                padding_pct: 0.5,
            }),
        };
        let out = g.compose(&base, &logo.image, &cfg).unwrap();
        // (35, 50) cae dentro del fondo (30..70) pero fuera del logo (40..60).
        let pix_fondo = out.get_pixel(35, 50).0;
        assert!(
            pix_fondo[0] < 20 && pix_fondo[1] < 20 && pix_fondo[2] < 20,
            "fondo debería ser ~negro, fue {pix_fondo:?}"
        );
    }

    #[test]
    fn gpu_y_cpu_coinciden_en_dimensiones() {
        let Some(_g) = skip_si_no_gpu() else {
            return;
        };
        let base = DynamicImage::ImageRgba8(imagen_base(80, 60, [10, 20, 30, 255]));
        let logo = logo_de(20, 20, [200, 200, 0, 200]);
        let cfg = LogoConfig {
            logo_id: "x".into(),
            position: LogoPosition { x: 0.7, y: 0.3 },
            scale_pct: 0.25,
            opacity: 0.8,
            background: None,
        };
        let gpu_out = compose_logo_gpu(base.clone(), &logo, &cfg).unwrap();
        let cpu_out = compose_logo(base, &logo, &cfg).unwrap();
        assert_eq!(gpu_out.dimensions(), cpu_out.dimensions());
    }
}
