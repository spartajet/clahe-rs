use std::cmp::min;
use image::*;
use imageproc::pixelops::interpolate;
use imageproc::stats::{histogram, ChannelHistogram};

pub fn clahe(input: GrayImage) -> Result<GrayImage, Box<dyn std::error::Error>> {
    let mut output = GrayImage::new(input.dimensions().0, input.dimensions().1);

    let tiles_hz = 8;
    let tiles_vt = 8;
    let tile_width = input.dimensions().0 / tiles_hz;
    let tile_height = input.dimensions().1 / tiles_vt;
    let mut lookup_tables = vec![vec![vec![0 as u8; 256]; tiles_hz as usize]; tiles_vt as usize];

    for (row_idx, row) in lookup_tables.iter_mut().enumerate() {
        for (col_idx, table) in row.iter_mut().enumerate() {
            let region_width = if col_idx == (tiles_hz - 1) as usize {
                tile_width + input.dimensions().0 % tiles_hz
            } else {
                tile_width
            };
            let region_height = if row_idx == (tiles_vt - 1) as usize {
                tile_height + input.dimensions().1 % tiles_vt
            } else {
                tile_height
            };

            let tile = SubImage::new(
                &input,
                tile_width * col_idx as u32,
                tile_height * row_idx as u32,
                region_width,
                region_height,
            );

            let tile_hist = clip_histogram(histogram(&tile.to_image()), 40);
            perform_gray_level_mapping(&tile_hist, table);
        }
    }

    for (x, y, val) in input.enumerate_pixels() {
        // use x and y to find four closest tile centers and their coordinates

        if let Ok(tile) = is_corner_region(
            x,
            y,
            tiles_hz,
            tiles_vt,
            input.dimensions().0,
            input.dimensions().1,
        ) {
            let output_val = lookup_tables[tile.y as usize][tile.x as usize][val.0[0] as usize];
            output.get_pixel_mut(x, y).0 = [output_val];
        } else if let Ok(tiles) = is_border_region(
            x,
            y,
            tiles_hz,
            tiles_vt,
            input.dimensions().0,
            input.dimensions().1,
        ) {
            let tile_pixel0 =
                lookup_tables[tiles.0.y as usize][tiles.0.x as usize][val.0[0] as usize];
            let tile_pixel1 =
                lookup_tables[tiles.1.y as usize][tiles.1.x as usize][val.0[0] as usize];
            let weight = if tiles.0.x == tiles.1.x {
                let tile_center0 = get_pixel_coordinate_from_tile_coordinate(tiles.0.x, tile_width);
                (x as f32 - tile_center0 as f32) / tile_width as f32
            } else if tiles.0.y == tiles.0.y {
                let tile_center0 =
                    get_pixel_coordinate_from_tile_coordinate(tiles.0.y, tile_height);
                (y as f32 - tile_center0 as f32) / tile_height as f32
            } else {
                0.0
            };

            output.get_pixel_mut(x, y).0 = if weight > 0.0 {
                interpolate(Luma::from([tile_pixel0]), Luma::from([tile_pixel1]), 1. - weight).0
            } else {
                interpolate(
                    Luma::from([tile_pixel1]),
                    Luma::from([tile_pixel0]),
                    -weight,
                )
                .0
            };
        } else {
            let tiles = get_neighbor_tiles(
                x,
                y,
                tiles_hz,
                tiles_vt,
                input.dimensions().0,
                input.dimensions().1,
            )
            .unwrap();

            let pixel_values = tiles.iter().map(|tile| lookup_tables[tile.y as usize][tile.x as usize][val.0[0] as usize]).collect::<Vec<u8>>();
            let x_weight = (x - (tiles[0].x * tile_width + (tile_width / 2))) as f32 / tile_width as f32;
            let y_weight = (y - (tiles[0].y * tile_height + (tile_height / 2))) as f32 / tile_height as f32;
            let intermediate_1 = interpolate(Luma::from([pixel_values[0]]), Luma::from([pixel_values[1]]), 1.0 - x_weight);
            let intermediate_2 = interpolate(Luma::from([pixel_values[3]]), Luma::from([pixel_values[2]]), 1.0 - x_weight);
            output.get_pixel_mut(x, y).0 = interpolate::<Luma<u8>>(intermediate_1, intermediate_2, 1.0 - y_weight).0;
        }
    }

    Ok(output)
}

fn clip_histogram(mut histogram: ChannelHistogram, limit: u32) -> ChannelHistogram {
    let mut num_pixels_over_limit: u32 = 0;

    if histogram.channels.len() != 1 {
        panic!("Too many channels!")
    }

    for (_bin, count) in histogram.channels[0].iter_mut().enumerate() {
        if *count > limit {
            num_pixels_over_limit += *count - limit;
            *count = limit;
        }
    }

    let excess_pixels_per_bin = num_pixels_over_limit / 256;

    for count in histogram.channels[0].iter_mut() {
        *count += excess_pixels_per_bin;
    }

    histogram
}

fn perform_gray_level_mapping(histogram: &ChannelHistogram, lookup_table: &mut Vec<u8>) {
    let num_pixels: u32 = histogram.channels[0].iter().sum();

    let mut num_pixels_seen: u32 = 0;
    for (index, entry) in lookup_table.iter_mut().enumerate() {
        num_pixels_seen += histogram.channels[0][index];

        let percent_pixels_seen = num_pixels_seen as f64 / num_pixels as f64;
        *entry = (percent_pixels_seen * 255.0) as u8;
    }
}

#[derive(Copy, Clone, Debug)]
struct TileCoordinate {
    pub x: u32,
    pub y: u32,
}

fn is_corner_region(
    x: u32,
    y: u32,
    tiles_hz: u32,
    tiles_vt: u32,
    dim_x: u32,
    dim_y: u32,
) -> Result<TileCoordinate, ()> {
    let tile_width = dim_x / tiles_hz;
    let tile_height = dim_y / tiles_vt;

    if (x <= tile_width / 2) && (y <= tile_height / 2) {
        // Top-left corner
        Ok(TileCoordinate { x: 0, y: 0 })
    } else if x > ((tile_width * tiles_hz) - tile_width / 2) && y <= tile_height / 2 {
        // Top-right corner
        Ok(TileCoordinate {
            x: tiles_hz - 1,
            y: 0,
        })
    } else if x > ((tile_width * tiles_hz) - tile_width / 2)
        && y > ((tile_height * tiles_vt) - tile_height / 2)
    {
        // Bottom-right corner
        Ok(TileCoordinate {
            x: tiles_hz - 1,
            y: tiles_vt - 1,
        })
    } else if (x <= tile_width / 2) && y > ((tile_height * tiles_vt) - tile_height / 2) {
        // Bottom-left corner
        Ok(TileCoordinate {
            x: 0,
            y: tiles_vt - 1,
        })
    } else {
        Err(())
    }
}

fn is_border_region(
    x: u32,
    y: u32,
    tiles_hz: u32,
    tiles_vt: u32,
    dim_x: u32,
    dim_y: u32,
) -> Result<(TileCoordinate, TileCoordinate), ()> {
    let tile_width = dim_x / tiles_hz;
    let tile_height = dim_y / tiles_vt;

    if y <= (tile_height / 2) {
        // Top border
        let left_x = min((x - (tile_width / 2)) / tile_width, tiles_hz - 2);
        let right_x = left_x + 1;
        Ok((
            TileCoordinate { x: left_x, y: 0 },
            TileCoordinate { x: right_x, y: 0 },
        ))
    } else if y > ((tiles_vt * tile_height) - (tile_height / 2)) {
        // Bottom border
        let left_x = min((x - (tile_width / 2)) / tile_width, tiles_hz - 2);
        let right_x = left_x + 1;
        Ok((
            TileCoordinate {
                x: left_x,
                y: tiles_vt - 1,
            },
            TileCoordinate {
                x: right_x,
                y: tiles_vt - 1,
            },
        ))
    } else if x <= (tile_width / 2) {
        // Left border
        let top_y = min((y - (tile_height / 2)) / tile_height, tiles_vt - 2);
        let bottom_y = top_y + 1;
        Ok((
            TileCoordinate { x: 0, y: top_y },
            TileCoordinate { x: 0, y: bottom_y },
        ))
    } else if x > ((tiles_hz * tile_width) - (tile_width / 2)) {
        // Right border
        let top_y = min((y - (tile_height / 2)) / tile_height, tiles_vt - 2);
        let bottom_y = top_y + 1;
        Ok((
            TileCoordinate {
                x: tiles_hz - 1,
                y: top_y,
            },
            TileCoordinate {
                x: tiles_hz - 1,
                y: bottom_y,
            },
        ))
    } else {
        Err(())
    }
}

fn get_neighbor_tiles(
    x: u32,
    y: u32,
    tiles_hz: u32,
    tiles_vt: u32,
    dim_x: u32,
    dim_y: u32,
) -> Result<[TileCoordinate; 4], ()> {
    let tile_width = dim_x / tiles_hz;
    let tile_height = dim_y / tiles_vt;

    let left_x = min((x - (tile_width / 2)) / tile_width, tiles_hz - 2);
    let right_x = left_x + 1;
    let top_y = min((y - (tile_height / 2)) / tile_height, tiles_vt - 2);
    let bottom_y = top_y + 1;

    Ok([
        TileCoordinate {
            x: left_x,
            y: top_y,
        },
        TileCoordinate {
            x: right_x,
            y: top_y,
        },
        TileCoordinate {
            x: right_x,
            y: bottom_y,
        },
        TileCoordinate {
            x: left_x,
            y: bottom_y,
        },
    ])
}

fn get_pixel_coordinate_from_tile_coordinate(tile_coord: u32, pixels_per_tile: u32) -> u32 {
    (pixels_per_tile / 2) + (tile_coord * pixels_per_tile)
}
