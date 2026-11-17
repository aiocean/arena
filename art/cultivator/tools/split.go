//usr/bin/env go run "$0" "$@"; exit "$?"
// split: cuts a sprite into rig parts by polygons in a spec, writes each part
// cropped to its box, a rig.json (box, pivot, parent, z) and a colour map.
// Usage: split.go sprite.png spec.json outdir
//
// spec.json: {"parts":[{"name","parent","z","pivot":[x,y],"poly":[[x,y],...],"source"}]}
// Parts are tested in file order and the first polygon holding a pixel takes
// it; a part with no poly takes whatever is left in its "rows" [y0,y1). A part
// with a "source" (a path relative to the spec) is cut from that picture
// instead, competing only with the parts cut from the same picture: that is
// how a part painted over (an arm) and the part repainted under it (the
// chest) both come out whole.
package main

import (
	"encoding/json"
	"fmt"
	"image"
	"image/color"
	"image/draw"
	"image/png"
	"os"
	"path/filepath"
)

type Part struct {
	Name   string       `json:"name"`
	Parent string       `json:"parent"`
	Z      int          `json:"z"`
	Pivot  [2]float64   `json:"pivot"`
	Poly   [][2]float64 `json:"poly,omitempty"`
	Rows   [2]int       `json:"rows,omitempty"`
	Source string       `json:"source,omitempty"`
}

type Out struct {
	Name   string     `json:"name"`
	Parent string     `json:"parent"`
	Z      int        `json:"z"`
	File   string     `json:"file"`
	Box    [4]int     `json:"box"`   // x, y, w, h on the source canvas
	Pivot  [2]float64 `json:"pivot"` // on the source canvas
	Local  [2]float64 `json:"local_pivot"`
	Pixels int        `json:"pixels"`
}

func inside(poly [][2]float64, x, y float64) bool {
	in := false
	for i, j := 0, len(poly)-1; i < len(poly); j, i = i, i+1 {
		xi, yi, xj, yj := poly[i][0], poly[i][1], poly[j][0], poly[j][1]
		if (yi > y) != (yj > y) && x < (xj-xi)*(y-yi)/(yj-yi)+xi {
			in = !in
		}
	}
	return in
}

func must(err error) {
	if err != nil {
		panic(err)
	}
}

func load(path string) image.Image {
	f, err := os.Open(path)
	must(err)
	defer f.Close()
	img, err := png.Decode(f)
	must(err)
	return img
}

var tints = []color.RGBA{
	{0, 220, 255, 255}, {255, 150, 0, 255}, {150, 90, 40, 255}, {255, 40, 60, 255}, {255, 110, 170, 255},
	{255, 200, 220, 255}, {160, 80, 255, 255}, {40, 90, 255, 255}, {120, 170, 255, 255}, {30, 160, 60, 255},
	{130, 230, 100, 255}, {200, 255, 120, 255}, {255, 255, 0, 255}, {0, 255, 180, 255}, {220, 220, 220, 255},
}

func main() {
	if len(os.Args) != 4 {
		fmt.Fprintln(os.Stderr, "usage: split.go sprite.png spec.json outdir")
		os.Exit(2)
	}
	var spec struct{ Parts []Part }
	raw, err := os.ReadFile(os.Args[2])
	must(err)
	must(json.Unmarshal(raw, &spec))
	outDir := os.Args[3]
	must(os.MkdirAll(outDir, 0o755))

	// One owner map per picture; parts compete only within their picture.
	sources := map[string]image.Image{"": load(os.Args[1])}
	for _, p := range spec.Parts {
		if _, ok := sources[p.Source]; !ok {
			sources[p.Source] = load(filepath.Join(filepath.Dir(os.Args[2]), p.Source))
		}
	}
	b := sources[""].Bounds()
	owner := map[string][]int{}
	for key, src := range sources {
		if src.Bounds() != b {
			panic(fmt.Sprintf("%s is %v, not %v", key, src.Bounds(), b))
		}
		own := make([]int, b.Dx()*b.Dy())
		orphans := 0
		for y := b.Min.Y; y < b.Max.Y; y++ {
			for x := b.Min.X; x < b.Max.X; x++ {
				k := (y-b.Min.Y)*b.Dx() + (x - b.Min.X)
				own[k] = -1
				if _, _, _, a := src.At(x, y).RGBA(); a == 0 {
					continue
				}
				px, py := float64(x)+0.5, float64(y)+0.5
				for i, p := range spec.Parts {
					if p.Source != key {
						continue
					}
					hit := false
					if len(p.Poly) > 0 {
						hit = inside(p.Poly, px, py)
					} else {
						hit = y >= p.Rows[0] && y < p.Rows[1]
					}
					if hit {
						own[k] = i
						break
					}
				}
				if own[k] < 0 && key == "" {
					orphans++
				}
			}
		}
		owner[key] = own
		if orphans > 0 {
			fmt.Printf("WARNING: %d painted pixels in no part\n", orphans)
		}
	}

	// The map shows every part where it sits, lowest z first.
	cmap := image.NewRGBA(b)
	var outs []Out
	for i, p := range spec.Parts {
		src, own := sources[p.Source], owner[p.Source]
		box := image.Rectangle{}
		n := 0
		for y := b.Min.Y; y < b.Max.Y; y++ {
			for x := b.Min.X; x < b.Max.X; x++ {
				if own[(y-b.Min.Y)*b.Dx()+(x-b.Min.X)] == i {
					box = box.Union(image.Rect(x, y, x+1, y+1))
					n++
				}
			}
		}
		if n == 0 {
			fmt.Printf("%-16s empty\n", p.Name)
			continue
		}
		img := image.NewRGBA(image.Rect(0, 0, box.Dx(), box.Dy()))
		for y := box.Min.Y; y < box.Max.Y; y++ {
			for x := box.Min.X; x < box.Max.X; x++ {
				if own[(y-b.Min.Y)*b.Dx()+(x-b.Min.X)] == i {
					img.Set(x-box.Min.X, y-box.Min.Y, src.At(x, y))
				}
			}
		}
		name := p.Name + ".png"
		w, err := os.Create(filepath.Join(outDir, name))
		must(err)
		must(png.Encode(w, img))
		w.Close()
		outs = append(outs, Out{p.Name, p.Parent, p.Z, name,
			[4]int{box.Min.X, box.Min.Y, box.Dx(), box.Dy()}, p.Pivot,
			[2]float64{p.Pivot[0] - float64(box.Min.X), p.Pivot[1] - float64(box.Min.Y)}, n})
		fmt.Printf("%-16s %-16s box %3d,%3d %3dx%-3d pivot %5.1f,%5.1f  %4d px\n",
			p.Name, p.Parent, box.Min.X, box.Min.Y, box.Dx(), box.Dy(), p.Pivot[0], p.Pivot[1], n)
	}
	for z := -10; z <= 10; z++ {
		for i, o := range outs {
			if o.Z != z {
				continue
			}
			tint := &image.Uniform{tints[i%len(tints)]}
			part := load(filepath.Join(outDir, o.File))
			r := image.Rect(o.Box[0], o.Box[1], o.Box[0]+o.Box[2], o.Box[1]+o.Box[3])
			draw.DrawMask(cmap, r, tint, image.Point{}, part, image.Point{}, draw.Over)
		}
	}
	for _, o := range outs {
		cmap.Set(int(o.Pivot[0]), int(o.Pivot[1]), color.RGBA{255, 255, 255, 255})
	}
	w, err := os.Create(filepath.Join(outDir, "_map.png"))
	must(err)
	must(png.Encode(w, cmap))
	w.Close()
	js, _ := json.MarshalIndent(map[string]any{
		"source": os.Args[1], "canvas": [2]int{b.Dx(), b.Dy()}, "parts": outs,
	}, "", "  ")
	must(os.WriteFile(filepath.Join(outDir, "rig.json"), js, 0o644))
}
