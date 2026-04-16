// Pimoroni Pico Display Pack Facade
// All dimensions in mm

// Global parameters
$fn = 60;
plate_thickness = 2.0;
extra_clearance = 0.2; // Tolerance for cutouts
// TBC
pcb_thickness = 1.0;

// Board Dimensions
board_width = 24.4; // Based on drawing (approx 24.4mm + margin)
board_height = 52.0; // Based on drawing (52mm total height)

// Screen Cutout (centered horizontally and vertically)
screen_width = 31.0;
screen_height = 17.6;

// Button dimensions
button_width = 3.2;
button_height = 4.3;

// Button positions (relative to bottom-left corner)
// Calculated from drawing center lines
buttons = [
  [7.70, 43.39], // Button B (Top Left)
  [16.70, 43.39], // Button A (Top Right)
  [7.70, 8.60], // Button Y (Bottom Left)
  [16.70, 8.60], // Button X (Bottom Right)
];

// LED width, height
led_dim = [2.7, 3.2];
// LED position (centered, relative to bottom left corner)
led_pos = [4.0, board_height / 2];

module pico_display_facade(additional_border_w) {
  difference() {
    union() {
      // Main Plate
      linear_extrude(height=plate_thickness)
        offset(r=extra_clearance)
          square([board_width, board_height]);
      // Additional width - for back to screw into
      translate([-additional_border_w / 2, -additional_border_w / 2, 0.1]) 
      linear_extrude(height=plate_thickness - 0.1) //-0.1 for visual impact only
        offset(r=extra_clearance)
          square([board_width + additional_border_w, board_height + additional_border_w]);
    }

    // Screen Cutout
    translate([(board_width - screen_height) / 2, (board_height - screen_width) / 2, -1])
      linear_extrude(height=plate_thickness + 2)
        offset(r=extra_clearance)
          square([screen_height, screen_width]);

    // Button cutouts
    for (pos = buttons) {
      translate([pos[0], pos[1], -1])
        linear_extrude(height=plate_thickness + 2)
          offset(r=extra_clearance)
            square(size=[button_height, button_width], center=true);
    }

    // LED Cutout
    translate([(board_width - led_dim[0]) / 2, 4, -1])
      linear_extrude(height=plate_thickness + 10)
        offset(r=extra_clearance)
          square(led_dim);
  }
}

module pico_display_backing(additional_border_width) {
  difference() {
    // Main Plate
    linear_extrude(height=plate_thickness)
      offset(r=extra_clearance)
        square([board_width + additional_border_width, board_height + additional_border_width]);

    // Pins cutout - 8 is a magic number atm
    translate([(board_width + additional_border_width - screen_height - 8) / 2, (board_height - additional_border_width - screen_width) / 2, -1])
      linear_extrude(height=plate_thickness + 2)
        offset(r=extra_clearance)
          square([board_width, board_height]);
  }
  leg_thickness = 5.0;
  // leg1
  linear_extrude(height=plate_thickness + pcb_thickness)
    square([leg_thickness, board_height]);
  // leg2
  translate([board_width - leg_thickness, 0, 0])
    linear_extrude(height=plate_thickness + pcb_thickness)
      square([leg_thickness, board_height]);
  // screw tabs
  screw_tab_length = 10;
  difference() {
    translate([1, -screw_tab_length, 0])
      linear_extrude(height=plate_thickness)
        square([board_width - 2, screw_tab_length]);
    // drill guide holes
    translate([5, -screw_tab_length / 2, -1])
      linear_extrude(height=plate_thickness + 2)
        circle(.4);
    translate([18, -screw_tab_length / 2, -1])
      linear_extrude(height=plate_thickness + 2)
        circle(.4);
    // countersinks
    translate([18, -screw_tab_length / 2, 0.1])
      cylinder(plate_thickness, r2=2, r1=0);
    translate([5, -screw_tab_length / 2, 0.1])
      cylinder(plate_thickness, r2=2, r1=0);
  }
}

pico_display_facade(10);
translate([-50, 0])
  pico_display_backing(10);
