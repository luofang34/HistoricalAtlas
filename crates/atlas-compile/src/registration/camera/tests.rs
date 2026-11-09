use super::*;
use crate::{
    input::LedgerInput,
    registration::{RasterFrame, RegistrationMethod, TargetFrame, register},
};
use nalgebra::{Matrix3, Rotation3, Vector3};

#[cfg(test)]
const RADIUS: f64 = 6_378_137.0;

/// 6 km of Web Mercator terrain: a coastal plain and a 150 m ridge to the south.
#[cfg(test)]
fn grid() -> ElevationGrid {
    let (width, height, cell_m) = (200, 200, 30.0);
    let west_m = RADIUS * 160.02_f64.to_radians();
    let north_m = RADIUS * (-9.40_f64).to_radians().tan().asinh();
    let elevations_m = (0..height)
        .flat_map(|row| {
            (0..width).map(move |column| {
                let (x, y) = (f64::from(column) * cell_m, f64::from(row) * cell_m);
                let ridge = 150.0 * (-((y - 4200.0 + 0.3 * x) / 600.0).powi(2)).exp();
                (8.0 + ridge).round() as i16
            })
        })
        .collect();
    ElevationGrid {
        width: width as usize,
        height: height as usize,
        west_m,
        north_m,
        cell_m,
        elevations_m,
    }
}

#[cfg(test)]
fn input() -> LedgerInput {
    let catalog = serde_json::from_value(serde_json::json!({"sources":[
        {"id":"oblique","title":"Test oblique","capture":{"medium":"aerial_photograph","sha256":"b".repeat(64)}},
        {"id":"terrain","title":"Test terrain","url":"https://example.org/terrain","rights":"Public domain test fixture"},
        {"id":"reference","title":"Test reference","rights":"Public domain test fixture","capture":{"sha256":"c".repeat(64)}}
    ]}))
    .unwrap();
    LedgerInput {
        catalog,
        data: Default::default(),
        aliases: Default::default(),
        published: Default::default(),
        derivations: Default::default(),
        terrains: Default::default(),
        traces: Vec::new(),
        place_maps: Default::default(),
        plate_models: Default::default(),
    }
}

#[cfg(test)]
fn truth(frame: &LocalFrame) -> (PinholeCamera, CameraPose) {
    let (heading, depression) = (160.0_f64.to_radians(), 45.0_f64.to_radians());
    let forward = Vector3::new(
        heading.sin() * depression.cos(),
        heading.cos() * depression.cos(),
        -depression.sin(),
    );
    let right = Vector3::new(heading.cos(), -heading.sin(), 0.0);
    let down = forward.cross(&right);
    let centre = frame.to_local(GeodeticPoint {
        position: LonLat {
            longitude: 160.045,
            latitude: -9.415,
        },
        height_m: 250.0,
    });
    (
        PinholeCamera {
            focal_px: 1800.0,
            principal: PlanarPoint {
                x: 1199.5,
                y: 899.5,
            },
        },
        CameraPose {
            centre,
            world_to_camera: Rotation3::from_matrix_unchecked(Matrix3::from_rows(&[
                right.transpose(),
                down.transpose(),
                forward.transpose(),
            ])),
        },
    )
}

#[cfg(test)]
fn control(name: &str, pixel: PlanarPoint, frame: &LocalFrame) -> ReferencedControl {
    let (camera, pose) = truth(frame);
    let ground = intersect(frame, &grid(), &camera, &pose, pixel).unwrap();
    ReferencedControl {
        basis: Vec::new(),
        landmark: name.into(),
        reference: "Synthetic terrain scene".into(),
        reference_sha256: Some("c".repeat(64)),
        uncertainty: None,
        point: ControlPoint {
            source: pixel,
            target: PlanarPoint {
                x: ground.position.longitude,
                y: ground.position.latitude,
            },
        },
    }
}

#[cfg(test)]
fn pixel(x: f64, y: f64) -> PlanarPoint {
    PlanarPoint { x, y }
}

#[cfg(test)]
fn job(frame: &LocalFrame, target_frame: TargetFrame) -> RegistrationJob {
    let points = [
        (150.0, 500.0),
        (2250.0, 450.0),
        (2200.0, 1700.0),
        (250.0, 1750.0),
        (1200.0, 1100.0),
    ];
    RegistrationJob {
        source_id: "oblique".into(),
        scan_page: None,
        source_frame: RasterFrame {
            width: 2400,
            height: 1800,
        },
        target_frame,
        method: RegistrationMethod::Camera { focal_px: None },
        controls: points
            .iter()
            .enumerate()
            .map(|(index, (x, y))| control(&format!("c{index}"), pixel(*x, *y), frame))
            .collect(),
        check_points: vec![control("check", pixel(800.0, 1400.0), frame)],
        vertices: vec![pixel(1700.0, 900.0)],
        terrain: Some(TerrainInput::Inline(Box::new(RegistrationTerrain {
            source_id: "terrain".into(),
            tiles: vec![TerrainTile {
                url: "https://example.org/terrain/1.tif".into(),
                sha256: "d".repeat(64),
            }],
            grid: grid(),
            water_surface_m: None,
        }))),
    }
}

#[test]
fn camera_registration_recovers_the_photograph_and_projects_vertices_onto_terrain() {
    let frame = LocalFrame::new(GeodeticPoint {
        position: LonLat {
            longitude: 160.05,
            latitude: -9.43,
        },
        height_m: 0.0,
    });
    let mut job = job(
        &frame,
        TargetFrame {
            datum: None,
            crs: "OGC:CRS84".into(),
            units: "degrees".into(),
        },
    );
    let result = register(&input(), job.clone()).unwrap();
    let camera = result.camera.as_ref().unwrap();
    assert!(
        (camera.focal_px - 1800.0).abs() < 2.0,
        "{}",
        camera.focal_px
    );
    assert!(
        (camera.centre[2] - 250.0).abs() < 2.0,
        "{:?}",
        camera.centre
    );
    assert!(camera.check_residuals_px.iter().all(|error| *error < 0.5));
    let expected = control("vertex", pixel(1700.0, 900.0), &frame).point.target;
    let found = result.vertices[0];
    // 1e-5 degrees is about a metre.
    assert!((found.x - expected.x).abs() < 1e-5 && (found.y - expected.y).abs() < 1e-5);
    job.terrain = None;
    let error = register(&input(), job).unwrap_err();
    assert!(error.to_string().contains("terrain grid"), "{error}");
}

#[test]
fn submerged_samples_are_seen_at_the_water_surface() {
    let mut seafloor = grid();
    seafloor
        .elevations_m
        .iter_mut()
        .for_each(|value| *value = -40);
    let position = LonLat {
        longitude: 160.05,
        latitude: -9.43,
    };
    let dry = VisibleSurface {
        grid: seafloor.clone(),
        water_surface_m: None,
        spacing_scale: 1.0,
    };
    let wet = VisibleSurface {
        grid: seafloor,
        water_surface_m: Some(0.0),
        spacing_scale: 1.0,
    };
    assert_eq!(dry.height_m(position), Some(-40.0));
    assert_eq!(wet.height_m(position), Some(0.0));
}

#[test]
fn lunar_camera_registration_uses_the_moon_ellipsoid() {
    let moon = BodyId::Moon.ellipsoid();
    let frame = LocalFrame::with_ellipsoid(
        moon,
        GeodeticPoint {
            position: LonLat {
                longitude: 160.05,
                latitude: -9.43,
            },
            height_m: 0.0,
        },
    );
    let lunar = TargetFrame {
        datum: Some(Datum::MoonIau),
        crs: "IAU_2015:30100".into(),
        units: "degrees".into(),
    };
    let result = register(&input(), job(&frame, lunar.clone())).unwrap();
    let camera = result.camera.as_ref().unwrap();
    assert!(
        (camera.focal_px - 1800.0).abs() < 2.0,
        "{}",
        camera.focal_px
    );
    assert!(
        (camera.centre[2] - 250.0).abs() < 2.0,
        "{:?}",
        camera.centre
    );
    let unspecified = TargetFrame {
        datum: Some(Datum::Unspecified),
        ..lunar
    };
    let error = register(&input(), job(&frame, unspecified)).unwrap_err();
    assert!(error.to_string().contains("body datum"), "{error}");
}

#[test]
fn shared_terrain_documents_register_like_embedded_grids() {
    let frame = LocalFrame::new(GeodeticPoint {
        position: LonLat {
            longitude: 160.05,
            latitude: -9.43,
        },
        height_m: 0.0,
    });
    let wgs84 = TargetFrame {
        datum: None,
        crs: "OGC:CRS84".into(),
        units: "degrees".into(),
    };
    let mut shared = job(&frame, wgs84);
    let Some(TerrainInput::Inline(terrain)) = shared.terrain.take() else {
        panic!("test job embeds its terrain");
    };
    let mut input = input();
    input.terrains.insert("test/area".into(), *terrain);
    shared.terrain = Some(TerrainInput::Shared(SharedTerrain {
        terrain_id: "test/area".into(),
    }));
    let json = serde_json::to_value(&shared).unwrap();
    assert_eq!(
        json["terrain"],
        serde_json::json!({"terrain_id": "test/area"})
    );
    let result = register(&input, shared.clone()).unwrap();
    assert!((result.camera.unwrap().focal_px - 1800.0).abs() < 2.0);
    shared.terrain = Some(TerrainInput::Shared(SharedTerrain {
        terrain_id: "test/missing".into(),
    }));
    let error = register(&input, shared).unwrap_err();
    assert!(
        error.to_string().contains("unknown terrain document"),
        "{error}"
    );
}
