//usr/bin/env go run "$0" "$@"; exit "$?"
// holes: takes rig parts off a sprite to show what lies under them. Writes
// the sprite without those parts and a mask (white = cleared, grown by
// -grow pixels) for an inpainting model to paint what the parts covered.
// Usage: holes.go [-grow 1] sprite.png rigdir out.png mask.png part...
package main

import (
	"encoding/json"
	"flag"
	"image"
	"image/color"
	"image/draw"
	"image/png"
	"os"
	"path/filepath"
)

type Out struct {
	Name string `json:"name"`
	File string `json:"file"`
	Box  [4]int `json:"box"`
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

func save(path string, img image.Image) {
	f, err := os.Create(path)
	must(err)
	defer f.Close()
	must(png.Encode(f, img))
}

func main() {
	grow := flag.Int("grow", 1, "grow the mask by this many pixels")
	flag.Parse()
	a := flag.Args()
	if len(a) < 5 {
		println("usage: holes.go [-grow 1] sprite.png rigdir out.png mask.png part...")
		os.Exit(2)
	}
	src := load(a[0])
	var rig struct{ Parts []Out }
	raw, err := os.ReadFile(filepath.Join(a[1], "rig.json"))
	must(err)
	must(json.Unmarshal(raw, &rig))
	take := map[string]bool{}
	for _, n := range a[4:] {
		take[n] = true
	}

	b := src.Bounds()
	out := image.NewRGBA(b)
	draw.Draw(out, b, src, b.Min, draw.Src)
	cleared := make([]bool, b.Dx()*b.Dy())
	for _, p := range rig.Parts {
		if !take[p.Name] {
			continue
		}
		part := load(filepath.Join(a[1], p.File))
		pb := part.Bounds()
		for y := 0; y < pb.Dy(); y++ {
			for x := 0; x < pb.Dx(); x++ {
				if _, _, _, al := part.At(x, y).RGBA(); al > 0 {
					sx, sy := p.Box[0]+x, p.Box[1]+y
					out.Set(sx, sy, color.RGBA{})
					cleared[sy*b.Dx()+sx] = true
				}
			}
		}
	}
	mask := image.NewGray(b)
	for y := 0; y < b.Dy(); y++ {
		for x := 0; x < b.Dx(); x++ {
			for dy := -*grow; dy <= *grow; dy++ {
				for dx := -*grow; dx <= *grow; dx++ {
					nx, ny := x+dx, y+dy
					if nx >= 0 && ny >= 0 && nx < b.Dx() && ny < b.Dy() && cleared[ny*b.Dx()+nx] {
						mask.SetGray(x, y, color.Gray{255})
					}
				}
			}
		}
	}
	save(a[2], out)
	save(a[3], mask)
}
