"""Open alignment candidates; a listed model is still measured and can fail.

The practical probe is sampled. Unresolved models and the complete map coverage
stay in its report; this short list highlights the largest retained leads.
Locations are placement origins or witness coordinates, as each entry describes. Historical
all-body flags require scene-composition and active-contact confirmation before
they can be described as reachable physical gaps. No item is an exemption.
"""
KNOWN = [
    dict(map='bf2_tat3', geometry='tat3_bldg_torture_grate', sources=['xbox','pc1'],
         location=[-90.,4.,125.], prior_extra_m=23.858722,
         status='resolved_model_partition', full_scene_extra_m=.000249,
         reason='historical 23.858722m model-local flag resolves against overlapping room art; full-scene sample .000249m, not an active offender'),
    dict(map='bf2_pol1', geometry='pol_bldg_hanger_dockingbay', sources=['xbox','pc1'],
         location=[93.87456,7.408475,-114.056204], prior_extra_m=.869186,
         reason='full-base-scene sample below lower dock frame; opacity/LOD and walking path not validated'),
    dict(map='bf2_spa3', geometry='cis_fly_fedcruiser', sources=['xbox','pc1'],
         location=[120.556833,84.956898,111.201488], prior_extra_m=5.410412,
         status='fixed_scoped', after_extra_m=.000346,
         reason='missing opaque HIGH hull panels recovered from the same Xbox LOWD resource on spa3/6/7; this full-scene witness improves to .000346m extra; other hull topology and alpha remain open'),
    dict(map='bf2_uta1', geometry='uta1_bldg_rep_ground', sources=['xbox','pc1'],
         location=[-217.896622,36.019970,190.045517], prior_extra_m=2.628084,
         reason='full-base-scene cliff-top sample; reachability/bounds, opacity and LOD not confirmed'),
    dict(map='bf2_myg1', geometry='myg1_sky_mountains', sources=['xbox','pc1'],
         location=[224.04,-4.701,-208.993], prior_extra_m=9.017915,
         status='classified_decorative_sky', location_space='placement_origin',
         reason='all six small collision boxes and the historical reverse witness lie inside the enabled deathregion; normal spawn-eye scenes show decorative skyline differences, not a demonstrated reachable missing ledge; numeric flags remain, whole-map alignment is open'),
    dict(map='bf2_pol1', geometry='pol_bldg_hanger_01', sources=['xbox','pc1'],
         location=[-17.,23.,-20.], prior_extra_m=7.999449,
         status='fixed_scoped', after_extra_m=.000748,
         reason='four hanger platforms repaired by donor reassembly in babd353c; sampled after <=.000748m, whole-map maximum unknown'),
    dict(map='bf2_spa3', geometry='cis_fly_fedcruiser', sources=['xbox','pc1'],
         location=[39.272,42.675,644.497], prior_extra_m=5.023690,
         status='fixed_scoped', after_extra_m=.000346,
         reason='historical missing-panel candidate repaired by same-donor LOWD segment recovery on spa3/6/7; whole-hull maximum and attachments remain unverified'),
    dict(map='bf1_bes2', geometry='bes2_bldg_skyway_ramp', sources=['pc1'],
         location=[-95.568,-34.061,121.123], prior_extra_m=3.006476,
         reason='historical per-model candidate; active contact and scene coverage need confirmation'),
    dict(map='bf1_bes2', geometry='bes2_bldg_chamber', sources=['xbox'],
         location=[-62.999710,11.999918,19.732536], prior_extra_m=2.924480, location_space='local',
         reason='current per-model diagnostic: PS2 residual .000008287m, donor 2.924488m; full-scene reachability not confirmed'),
    dict(map='bf1_nab2', geometry='nab2_bldg_b_19', sources=['pc1'],
         location=[-7.702659,.119198,-24.528064], prior_extra_m=1.570144, location_space='local',
         reason='current per-model diagnostic: PS2 residual16.721405m, donor18.291549m; full-scene reachability not confirmed'),
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

# These fourteen source/map rows are material limitations, not measured contact
# gaps. Their entire authored water sheets retain a flat native tint because the
# existing water shader does not consume the donor normal-map animation.
for _map, _level, _sources in (
    ('bf1_kas1', -45., ['xbox']), ('bf1_kas2', -5., ['xbox']),
    ('bf1_nab2', -54., ['xbox']), ('bf1_yav1', -72.5, ['xbox']),
    ('bf2_dag1', 1., ['xbox','pc1']), ('bf2_dea1', 41.75, ['xbox','pc1']),
    ('bf2_kas2', 1., ['xbox','pc1']), ('bf2_nab2', 5., ['xbox','pc1']),
    ('bf2_yav1', -72.5, ['xbox','pc1']),
):
    KNOWN.append(dict(map=_map, geometry='water_sheet', sources=_sources,
                      location='entire authored sheet', water_level_m=_level,
                      prior_extra_m=None, status='material_limitation',
                      reason='no resolvable diffuse MainTexture; donor normal animation is not consumed; native tint fallback stays unchanged'))
