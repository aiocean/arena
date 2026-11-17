//usr/bin/env go run "$0" "$@"; exit "$?"
// assemble: finishes a rig split by split.go and proves it by posing it.
//
//   - the far arm: the near arm's three parts, darkened, set behind the body
//   - the sword: a separate picture, flipped so the blade points forward
//     (+x), its grip pinned to the near hand
//   - poses.png: the rig in a few fighting poses, each part turned round its
//     pivot and carried by its parent (nearest-neighbour, so pixels stay
//     whole; the game does the real turning)
//
// Usage: assemble.go rigdir sword.png gripX,gripY
package main

import (
	"encoding/json"
	"fmt"
	"image"
	"image/color"
	"image/draw"
	"image/png"
	"math"
	"os"
	"path/filepath"
	"strconv"
	"strings"
)

type Out struct {
	Name   string     `json:"name"`
	Parent string     `json:"parent"`
	Z      int        `json:"z"`
	File   string     `json:"file"`
	Box    [4]int     `json:"box"`
	Pivot  [2]float64 `json:"pivot"`
	Local  [2]float64 `json:"local_pivot"`
	Pixels int        `json:"pixels"`
	Note   string     `json:"note,omitempty"`
}

type Rig struct {
	Source string `json:"source"`
	Canvas [2]int `json:"canvas"`
	Parts  []Out  `json:"parts"`
}

func must(err error) {
	if err != nil {
		panic(err)
	}
}

func load(path string) *image.RGBA {
	f, err := os.Open(path)
	must(err)
	defer f.Close()
	img, err := png.Decode(f)
	must(err)
	out := image.NewRGBA(image.Rect(0, 0, img.Bounds().Dx(), img.Bounds().Dy()))
	draw.Draw(out, out.Bounds(), img, img.Bounds().Min, draw.Src)
	return out
}

func save(path string, img image.Image) {
	f, err := os.Create(path)
	must(err)
	defer f.Close()
	must(png.Encode(f, img))
}

func (r *Rig) part(name string) *Out {
	for i := range r.Parts {
		if r.Parts[i].Name == name {
			return &r.Parts[i]
		}
	}
	return nil
}

func (r *Rig) put(o Out) {
	if p := r.part(o.Name); p != nil {
		*p = o
		return
	}
	r.Parts = append(r.Parts, o)
}

// affine: x' = a*x + b*y + c, y' = d*x + e*y + f
type affine [6]float64

var identity = affine{1, 0, 0, 0, 1, 0}

func (m affine) mul(n affine) affine { // m after n
	return affine{
		m[0]*n[0] + m[1]*n[3], m[0]*n[1] + m[1]*n[4], m[0]*n[2] + m[1]*n[5] + m[2],
		m[3]*n[0] + m[4]*n[3], m[3]*n[1] + m[4]*n[4], m[3]*n[2] + m[4]*n[5] + m[5],
	}
}

func rotAbout(px, py, deg float64) affine {
	s, c := math.Sincos(deg * math.Pi / 180)
	return affine{c, -s, px - c*px + s*py, s, c, py - s*px - c*py}
}

func (m affine) inv() affine {
	det := m[0]*m[4] - m[1]*m[3]
	a, b, d, e := m[4]/det, -m[1]/det, -m[3]/det, m[0]/det
	return affine{a, b, -(a*m[2] + b*m[5]), d, e, -(d*m[2] + e*m[5])}
}

func pose(rig *Rig, dir string, angles map[string]float64, shift [2]float64) *image.RGBA {
	world := map[string]affine{}
	var resolve func(name string) affine
	resolve = func(name string) affine {
		if m, ok := world[name]; ok {
			return m
		}
		p := rig.part(name)
		parent := identity
		if p.Parent != "" {
			parent = resolve(p.Parent)
		} else {
			parent = affine{1, 0, shift[0], 0, 1, shift[1]}
		}
		m := parent.mul(rotAbout(p.Pivot[0], p.Pivot[1], angles[name]))
		world[name] = m
		return m
	}
	out := image.NewRGBA(image.Rect(0, 0, rig.Canvas[0], rig.Canvas[1]))
	for z := -10; z <= 10; z++ {
		for _, p := range rig.Parts {
			if p.Z != z {
				continue
			}
			img := load(filepath.Join(dir, p.File))
			inv := resolve(p.Name).inv()
			for y := 0; y < out.Bounds().Dy(); y++ {
				for x := 0; x < out.Bounds().Dx(); x++ {
					fx, fy := float64(x)+0.5, float64(y)+0.5
					sx := inv[0]*fx + inv[1]*fy + inv[2] - float64(p.Box[0])
					sy := inv[3]*fx + inv[4]*fy + inv[5] - float64(p.Box[1])
					ix, iy := int(math.Floor(sx)), int(math.Floor(sy))
					if ix < 0 || iy < 0 || ix >= img.Bounds().Dx() || iy >= img.Bounds().Dy() {
						continue
					}
					if c := img.RGBAAt(ix, iy); c.A > 0 {
						out.SetRGBA(x, y, c)
					}
				}
			}
		}
	}
	return out
}

func main() {
	if len(os.Args) != 4 {
		fmt.Fprintln(os.Stderr, "usage: assemble.go rigdir sword.png gripX,gripY")
		os.Exit(2)
	}
	dir := os.Args[1]
	raw, err := os.ReadFile(filepath.Join(dir, "rig.json"))
	must(err)
	var rig Rig
	must(json.Unmarshal(raw, &rig))

	// Far arm: the near arm in shadow, a step back and up, behind the body.
	far := map[string]string{"left_upper_arm": "right_upper_arm", "left_forearm": "right_forearm", "left_hand": "right_hand"}
	farParent := map[string]string{"right_upper_arm": "chest", "right_forearm": "right_upper_arm", "right_hand": "right_forearm"}
	const bx, by = -4, -1
	for near, name := range far {
		p := *rig.part(near)
		img := load(filepath.Join(dir, p.File))
		for i := 0; i < len(img.Pix); i += 4 {
			for k := 0; k < 3; k++ {
				img.Pix[i+k] = uint8(float64(img.Pix[i+k]) * 0.62)
			}
		}
		save(filepath.Join(dir, name+".png"), img)
		p.Name, p.File, p.Parent, p.Z = name, name+".png", farParent[name], -5
		p.Box[0] += bx
		p.Box[1] += by
		p.Pivot = [2]float64{p.Pivot[0] + bx, p.Pivot[1] + by}
		p.Note = "drawn from the near arm, darkened: hidden behind the body in the source"
		rig.put(p)
	}

	// Sword: crop, flip so the blade points +x, grip on the near hand.
	sw := load(os.Args[2])
	g := strings.Split(os.Args[3], ",")
	gx, _ := strconv.ParseFloat(g[0], 64)
	gy, _ := strconv.ParseFloat(g[1], 64)
	box := image.Rectangle{}
	for y := 0; y < sw.Bounds().Dy(); y++ {
		for x := 0; x < sw.Bounds().Dx(); x++ {
			if sw.RGBAAt(x, y).A > 0 {
				box = box.Union(image.Rect(x, y, x+1, y+1))
			}
		}
	}
	crop := image.NewRGBA(image.Rect(0, 0, box.Dx(), box.Dy()))
	n := 0
	for y := 0; y < box.Dy(); y++ {
		for x := 0; x < box.Dx(); x++ {
			c := sw.RGBAAt(box.Max.X-1-x, box.Min.Y+y)
			crop.SetRGBA(x, y, c)
			if c.A > 0 {
				n++
			}
		}
	}
	save(filepath.Join(dir, "sword.png"), crop)
	lgx, lgy := float64(box.Max.X-1)-gx, gy-float64(box.Min.Y)
	hand := rig.part("left_hand")
	hx, hy := hand.Pivot[0]+1, hand.Pivot[1]+6 // the fist, below the wrist
	rig.put(Out{
		Name: "sword", Parent: "left_hand", Z: 10, File: "sword.png",
		Box:   [4]int{int(math.Round(hx - lgx)), int(math.Round(hy - lgy)), box.Dx(), box.Dy()},
		Pivot: [2]float64{hx, hy}, Local: [2]float64{lgx, lgy}, Pixels: n,
		Note: "blade points +x at rest; turn it to aim",
	})

	js, _ := json.MarshalIndent(rig, "", "  ")
	must(os.WriteFile(filepath.Join(dir, "rig.json"), js, 0o644))

	// Proof: a row of poses. Angles in degrees, clockwise on screen.
	poses := []struct {
		name string
		a    map[string]float64
	}{
		{"rest", map[string]float64{"sword": 65}},
		{"guard", map[string]float64{"left_upper_arm": -55, "left_forearm": -60, "sword": 45,
			"right_upper_arm": -30, "right_forearm": -70}},
		{"thrust", map[string]float64{"chest": 8, "head": -6, "left_upper_arm": -80, "left_forearm": -5,
			"sword": 85, "right_upper_arm": 40, "right_forearm": 20, "left_thigh": -20, "left_shin": 15, "right_thigh": 15}},
		{"wind-up", map[string]float64{"chest": -6, "left_upper_arm": 150, "left_forearm": 30, "sword": 10,
			"right_upper_arm": -60, "right_forearm": -30}},
		{"overhead", map[string]float64{"left_upper_arm": -165, "left_forearm": -20, "sword": 85,
			"right_upper_arm": -150, "right_forearm": -30}},
		{"slash-end", map[string]float64{"chest": 10, "head": -4, "left_upper_arm": -40, "left_forearm": -20,
			"sword": 75, "right_upper_arm": 30, "left_thigh": -15, "right_thigh": 10}},
	}
	const pad = 40
	w, h := rig.Canvas[0]+2*pad, rig.Canvas[1]+pad
	sheet := image.NewRGBA(image.Rect(0, 0, len(poses)*(w+2), h))
	draw.Draw(sheet, sheet.Bounds(), &image.Uniform{color.RGBA{96, 96, 108, 255}}, image.Point{}, draw.Src)
	big := Rig{rig.Source, [2]int{w, h}, rig.Parts}
	for i, p := range poses {
		img := pose(&big, dir, p.a, [2]float64{pad, pad})
		draw.Draw(sheet, image.Rect(i*(w+2), 0, i*(w+2)+w, h), img, image.Point{}, draw.Over)
		fmt.Println("pose", p.name)
	}
	save(filepath.Join(dir, "_poses.png"), sheet)
	fmt.Println("wrote", filepath.Join(dir, "rig.json"), "and _poses.png")
}
