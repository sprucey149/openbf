"""Open alignment candidates; a listed model is still measured and can fail.

The practical probe is sampled. Unresolved models and the complete map coverage
stay in its report; this short list highlights the largest retained leads.
Locations are placement origins except the explicit bladder witness. Historical
all-body flags require scene-composition and active-contact confirmation before
they can be described as reachable physical gaps. No item is an exemption.
"""
KNOWN = [
    dict(map='bf2_tat3', geometry='tat3_bldg_torture_grate', sources=['xbox','pc1'],
         location=[-90.,4.,125.], prior_extra_m=23.858722,
         reason='historical all-body model sample; check active contact and overlapping room art'),
    dict(map='bf2_myg1', geometry='myg1_sky_mountains', sources=['xbox','pc1'],
         location=[224.04,-4.701,-208.993], prior_extra_m=9.017915,
         reason='historical all-body sky sample; reachability and visibility need confirmation'),
    dict(map='bf2_pol1', geometry='pol_bldg_hanger_01', sources=['xbox','pc1'],
         location=[-17.,23.,-20.], prior_extra_m=7.999449,
         reason='historical model partition flag; hanger_02/03/04 share this placement origin'),
    dict(map='bf2_spa3', geometry='cis_fly_fedcruiser', sources=['xbox','pc1'],
         location=[39.272,42.675,644.497], prior_extra_m=5.023690,
         reason='historical capital-ship sample; also occurs on spa6 and spa7; attachments remain open'),
    dict(map='bf1_bes2', geometry='bes2_bldg_skyway_ramp', sources=['pc1'],
         location=[-95.568,-34.061,121.123], prior_extra_m=3.006476,
         reason='historical per-model candidate; active contact and scene coverage need confirmation'),
    dict(map='bf2_fel1', geometry='fel1_bldg_bladder', sources=['xbox','pc1'],
         location=[46.889041533,25.871313944,46.496180138], prior_extra_m=.225374890,
         reason='current donor forward witness inside play at bldr21/base9; PS2 residual .143311886 m, donor .368686776 m'),
    dict(map='bf2_fel1', geometry='fel1_bldg_bladder', sources=['xbox','pc1'],
         location=[.409796,4.058988,.867501], prior_extra_m=.744994,
         reason='REJECTED fallback experiment, local witness; not the current donor gap and not an enabled recipe'),
    dict(map='bf1_rhn1', geometry='rhn1_prop_tunnel_height', sources=['xbox','pc1'],
         location=[-251.180718,1.060658,86.922573], prior_extra_m=.390603,
         reason='current per-object witness: Xbox .390602 m, PC1 .390520 m; scene overlap and full reachability unverified'),
    dict(map='bf1_rhn1', geometry='rhn1_prop_tunnel_t', sources=['xbox'],
         location=[-169.652061,13.125581,38.744931], prior_extra_m=.143715,
         reason='current per-object witness; no reviewed correction, possible scene overlap'),
    dict(map='bf1_rhn1', geometry='rhn1_prop_tunnel_t', sources=['pc1'],
         location=[-183.288242,9.931510,49.587692], prior_extra_m=.167696,
         reason='current per-object witness; no reviewed correction, possible scene overlap'),
    dict(map='bf1_rhn1', geometry='rhn1_prop_tunnel_straight_6', sources=['xbox','pc1'],
         location=[-187.165407,7.912724,65.662341], prior_extra_m=.178698,
         reason='current per-object witness: Xbox .178698 m, PC1 .178677 m; scene overlap and full reachability unverified'),
    dict(map='bf1_geo1/bf2_geo1', geometry='hailfire_missile_rack', sources=['xbox','pc1'],
         location=[1.60047575,3.93175332,-.09859701], prior_extra_m=.492089,
         reason='moving donor local witness; native .037697 m, donor .529787 m; settled world transform not captured'),
]
