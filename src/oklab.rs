//! The color arithmetic gradients and the color forms need, the same to the last bit as
//! PSWriteColorEX's in Windows PowerShell 5.1 and PowerShell 7: the constants are the bit patterns
//! of their doubles, and the arithmetic is +, -, * and / in the order PSWriteColorEX writes it,
//! which IEEE 754 rounds the same everywhere.

use crate::colors::round_even;

/// The bit patterns of the linear value of each sRGB channel value 0-255: ((c / 255 + 0.055) /
/// 1.055) ^ 2.4, or c / 255 / 12.92 at the dark end. It rises, so a linear value finds its
/// nearest channel value by binary search.
const SRGB_TO_LINEAR_BITS: [u64; 256] = [
    0x0000000000000000, 0x3F33E45677C176F7, 0x3F43E45677C176F7, 0x3F4DD681B3A23272,
    0x3F53E45677C176F7, 0x3F58DD6C15B1D4B4, 0x3F5DD681B3A23272, 0x3F6167CBA8C94818,
    0x3F63E45677C176F7, 0x3F6660E146B9A5D5, 0x3F68DD6C15B1D4B4, 0x3F6B6A31B5259C99,
    0x3F6E1E31D70C99DD, 0x3F707C38BF8583A9, 0x3F71FCC2BEED6421, 0x3F7390FFAF95E279,
    0x3F753936CC7BC928, 0x3F76F5ADDB50C915, 0x3F78C6A94031B561, 0x3F7AAC6C0FB97351,
    0x3F7CA7381F9F602B, 0x3F7EB74E160978D0, 0x3F806E76BBDA92B8, 0x3F818C2A5A8A8044,
    0x3F82B4E09B3F0AE3, 0x3F83E8B7B3BDE965, 0x3F8527CD60AF8B85, 0x3F86723EEA8D3709,
    0x3F87C8292A3DB6B3, 0x3F8929A88D67B521, 0x3F8A96D91A8016BD, 0x3F8C0FD67499FAB6,
    0x3F8D94BBDEFD740E, 0x3F8F25A44089883F, 0x3F9061551372C694, 0x3F9135F3E4C2CCE2,
    0x3F9210BB8642B172, 0x3F92F1B8C1AE46BD, 0x3F93D8F839B79C0B, 0x3F94C6866B3E9FA4,
    0x3F95BA6FAE794313, 0x3F96B4C0380D2DEE, 0x3F97B5841A1BF3AC, 0x3F98BCC74542ADDB,
    0x3F99CA95898DC8B5, 0x3F9ADEFA9761C020, 0x3F9BFA0200597BD9, 0x3F9D1BB7381AEC1F,
    0x3F9E442595227BCA, 0x3F9F73585185E1B5, 0x3FA054AD45D76878, 0x3FA0F31BA386FF26,
    0x3FA194FCB663747B, 0x3FA23A55E62A662A, 0x3FA2E32C8E148D11, 0x3FA38F85FD21EACF,
    0x3FA43F67766310FF, 0x3FA4F2D6313FA8D0, 0x3FA5A9D759BA5ED0, 0x3FA6647010B254EE,
    0x3FA722A56C2239EE, 0x3FA7E47C775D2427, 0x3FA8A9FA33494B07, 0x3FA973239698B9CC,
    0x3FAA3FFD8E001389, 0x3FAB108CFC6B7FBC, 0x3FABE4D6BB31D522, 0x3FACBCDF9A4616F2,
    0x3FAD98AC60675833, 0x3FAE7841CB4F16DF, 0x3FAF5BA48FDE2048, 0x3FB0216CAD240765,
    0x3FB096F2671EB815, 0x3FB10E65C38A5192, 0x3FB187C90BF8BCE2, 0x3FB2031E85F5D6DA,
    0x3FB28068731A1952, 0x3FB2FFA9111CB94B, 0x3FB380E299E53F92, 0x3FB40417439CA10F,
    0x3FB4894940BDDBFB, 0x3FB5107AC0261E59, 0x3FB599ADED247AAC, 0x3FB624E4EF892ED4,
    0x3FB6B221EBB4817E, 0x3FB7416702A539D1, 0x3FB7D2B65206B527, 0x3FB86611F43E9E6A,
    0x3FB8FB7C007A4A70, 0x3FB992F68ABBBC89, 0x3FBA2C83A3E6566D, 0x3FBAC82559CB3644,
    0x3FBB65DDB7354604, 0x3FBC05AEC3F4FE5E, 0x3FBCA79A84EBE030, 0x3FBD4BA2FC17A6A5,
    0x3FBDF1CA289D34B8, 0x3FBE9A1206D34003, 0x3FBF447C904CBB4E, 0x3FBFF10BBBE302C2,
    0x3FC04FE0BEDFE5F1, 0x3FC0A84FE3B36D8F, 0x3FC101D443DFC06F, 0x3FC15C6ED58EEFDF,
    0x3FC1B8208DA5FEF0, 0x3FC214EA5FC9514A, 0x3FC272CD3E610123, 0x3FC2D1CA1A9D1CFB,
    0x3FC331E1E479CDF5, 0x3FC393158AC3674E, 0x3FC3F565FB1A5FD5, 0x3FC458D421F735DF,
    0x3FC4BD60EAAE3E73, 0x3FC5230D3F736034, 0x3FC589DA095DBAA1, 0x3FC5F1C8306B3A3C,
    0x3FC65AD89B841A2B, 0x3FC6C50C307E53BF, 0x3FC73063D420FC80, 0x3FC79CE06A279303,
    0x3FC80A82D5453B5D, 0x3FC8794BF727EB3F, 0x3FC8E93CB07B8679, 0x3FC95A55E0ECEC0B,
    0x3FC9CC98672CF47E, 0x3FCA400520F3619C, 0x3FCAB49CEB01C003, 0x3FCB2A60A1263B0A,
    0x3FCBA1511E3E632D, 0x3FCC196F3C39E76F, 0x3FCC92BBD41D41FE, 0x3FCD0D37BE045851,
    0x3FCD88E3D1250F68, 0x3FCE05C0E3D1D3E0, 0x3FCE83CFCB7C16F0, 0x3FCF03115CB6BFD3,
    0x3FCF83866B38924D, 0x3FD00297E4EF4553, 0x3FD044072557177A, 0x3FD086115F6BEB3A,
    0x3FD0C8B6FB5C735E, 0x3FD10BF860EF039A, 0x3FD14FD5F782A5A6, 0x3FD1945026102997,
    0x3FD1D967532B31B1, 0x3FD21F1BE50339E7, 0x3FD2656E41649AE3, 0x3FD2AC5ECDB988F8,
    0x3FD2F3EDEF0B0ED8, 0x3FD33C1C0A020438, 0x3FD384E982E800B1, 0x3FD3CE56BDA84A81,
    0x3FD418641DD0C1BC, 0x3FD463120692C7AF, 0x3FD4AE60DAC4229D, 0x3FD4FA50FCDFDE15,
    0x3FD546E2CF0727A9, 0x3FD59416B3022858, 0x3FD5E1ED0A40DAAB, 0x3FD6306635DBDD7B,
    0x3FD67F82969543A2, 0x3FD6CF428CD96079, 0x3FD71FA678BF915D, 0x3FD770AEBA0B042A,
    0x3FD7C25BB02B7AC5, 0x3FD814ADBA3E0BD9, 0x3FD867A5370DE0B1, 0x3FD8BB428514F067,
    0x3FD90F86027CB84E, 0x3FD964700D1EF1B1, 0x3FD9BA0102864521, 0x3FDA10393FEEFAFD,
    0x3FDA67192247A9BE, 0x3FDABEA10631E195, 0x3FDB16D14802D5CA, 0x3FDB6FAA43C403BB,
    0x3FDBC92C5533D785, 0x3FDC2357D7C64E5D, 0x3FDC7E2D26A596DE, 0x3FDCD9AC9CB2AEF2,
    0x3FDD35D69485FFC5, 0x3FDD92AB686FF782, 0x3FDDF02B7279A10D, 0x3FDE4E570C6539C5,
    0x3FDEAD2E8FAEC526, 0x3FDF0CB2558C9EA4, 0x3FDF6CE2B6F00983, 0x3FDFCDC00C85BEC2,
    0x3FE017A5575B3CB2, 0x3FE048C17AD3C04B, 0x3FE07A349C9D9837, 0x3FE0ABFEE888C050,
    0x3FE0DE208A4444C8, 0x3FE11099AD5E83EB, 0x3FE1436A7D456EEF, 0x3FE176932546CA12,
    0x3FE1AA13D0906BDA, 0x3FE1DDECAA307B85, 0x3FE2121DDD15AECE, 0x3FE246A7940F86D1,
    0x3FE27B89F9CE8C4B, 0x3FE2B0C538E48B07, 0x3FE2E6597BC4CCA0, 0x3FE31C46ECC4528D,
    0x3FE3528DB61A0F73, 0x3FE3892E01DF1FCC, 0x3FE3C027FA0F01EB, 0x3FE3F77BC887CD3B,
    0x3FE42F29970A68F8, 0x3FE467318F3AC22D, 0x3FE49F93DAA00113, 0x3FE4D850A2A4BDE1,
    0x3FE51168109734E5, 0x3FE54ADA4DA97A1B, 0x3FE584A782F1AC23, 0x3FE5BECFD96A2698,
    0x3FE5F95379F1B3ED, 0x3FE634328D4BBE97, 0x3FE66F6D3C2081CF, 0x3FE6AB03AEFD39AA,
    0x3FE6E6F60E5452B1, 0x3FE72344827D98F6, 0x3FE75FEF33B6669B, 0x3FE79CF64A21D1E2,
    0x3FE7DA59EDC8DAB0, 0x3FE8181A469A9787, 0x3FE856377C6C6224, 0x3FE894B1B6FA0377,
    0x3FE8D3891DE5DF49, 0x3FE912BDD8B91F45, 0x3FE952500EE3DDA5, 0x3FE9923FE7BD4F67,
    0x3FE9D28D8A83EDFC, 0x3FEA13391E5DA09F, 0x3FEA5442CA57E52E, 0x3FEA95AAB567F88F,
    0x3FEAD771066AFEC2, 0x3FEB1995E4262A69, 0x3FEB5C197546E3F8, 0x3FEB9EFBE062F086,
    0x3FEBE23D4BF8981B, 0x3FEC25DDDE6ECBBB, 0x3FEC69DDBE154AF1, 0x3FECAE3D1124C90B,
    0x3FECF2FBFDBF11F1, 0x3FED381AA9EF2E82, 0x3FED7D993BA988D4, 0x3FEDC377D8CC0FD5,
    0x3FEE09B6A71E5AA6, 0x3FEE5055CC51CBB4, 0x3FEE97556E01B351, 0x3FEEDEB5B1B37216,
    0x3FEF2676BCD69ADE, 0x3FEF6E98B4C51466, 0x3FEFB71BBEC33AB2, 0x3FF0000000000000,
];

fn srgb_to_linear(channel: i64) -> f64 {
    f64::from_bits(SRGB_TO_LINEAR_BITS[channel.clamp(0, 255) as usize])
}

// Bjorn Ottosson's OKLab matrices: linear sRGB to LMS, the cube roots of LMS to OKLab, OKLab to
// the cube roots of LMS, and LMS to linear sRGB
const L1: f64 = f64::from_bits(0x3FDA61D629F2E197);
const L2: f64 = f64::from_bits(0x3FE129A2D9E60E32);
const L3: f64 = f64::from_bits(0x3FAA572112081026);
const M1: f64 = f64::from_bits(0x3FCB1FA76156A7C5);
const M2: f64 = f64::from_bits(0x3FE5C84A69936914);
const M3: f64 = f64::from_bits(0x3FBB7E5DF0497455);
const S1: f64 = f64::from_bits(0x3FB69AFD7A044C17);
const S2: f64 = f64::from_bits(0x3FD207AE728A2F45);
const S3: f64 = f64::from_bits(0x3FE428C9177A5EDB);
const LL: f64 = f64::from_bits(0x3FCAF02A3FE8A4FA);
const LM: f64 = f64::from_bits(0x3FE9655120032AAD);
const LS: f64 = -f64::from_bits(0x3F70ADD9BD572B38);
const AL: f64 = f64::from_bits(0x3FFFA5E1BFFFDE12);
const AM: f64 = -f64::from_bits(0x40036DC1BFFE5D3E);
const AS: f64 = f64::from_bits(0x3FDCD686FFF371A5);
const BL: f64 = f64::from_bits(0x3F9A869680B729E0);
const BM: f64 = f64::from_bits(0x3FE90C776001F502);
const BS: f64 = -f64::from_bits(0x3FE9E0AC0001353D);
const LA: f64 = f64::from_bits(0x3FD95D9920068C8A);
const LB: f64 = f64::from_bits(0x3FCB9F751FFA8CC8);
const MA: f64 = -f64::from_bits(0x3FBB06117FEEC881);
const MB: f64 = -f64::from_bits(0x3FB058BF3FE39E34);
const SA: f64 = -f64::from_bits(0x3FB6E86F5FDF38B5);
const SB: f64 = -f64::from_bits(0x3FF4A9ECBFFEAA8D);
const R1: f64 = f64::from_bits(0x40104E955DC3D73A);
const R2: f64 = -f64::from_bits(0x400A76317EA9DE73);
const R3: f64 = f64::from_bits(0x3FCD906C3222FFEF);
const G1: f64 = -f64::from_bits(0x3FF44B85A62C2AFF);
const G2: f64 = f64::from_bits(0x4004E0C87D01BF65);
const G3: f64 = -f64::from_bits(0x3FD5D82D4F5D4F2A);
const B1: f64 = -f64::from_bits(0x3F712FEA56E00671);
const B2: f64 = -f64::from_bits(0x3FE68267C131178D);
const B3: f64 = f64::from_bits(0x3FFB5263CAEF6BCD);

/// The cube root of a number from 0 to 1, by 40 Newton steps from 1, which use only *, + and /.
fn cube_root(value: f64) -> f64 {
    if value <= 0.0 {
        return 0.0;
    }
    let mut root = 1.0f64;
    for _ in 0..40 {
        root = (2.0 * root + value / (root * root)) / 3.0;
    }
    root
}

/// The OKLab coordinates L, a and b of an sRGB color.
pub fn to_oklab(rgb: [i64; 3]) -> [f64; 3] {
    let r = srgb_to_linear(rgb[0]);
    let g = srgb_to_linear(rgb[1]);
    let b = srgb_to_linear(rgb[2]);
    let l = cube_root(L1 * r + L2 * g + L3 * b);
    let m = cube_root(M1 * r + M2 * g + M3 * b);
    let s = cube_root(S1 * r + S2 * g + S3 * b);
    [LL * l + LM * m + LS * s, AL * l + AM * m + AS * s, BL * l + BM * m + BS * s]
}

/// The sRGB channel value 0-255 whose linear value is nearest the one given; the lower of two as
/// near. Below 0 is 0 and above 1 is 255.
fn from_linear(value: f64) -> i64 {
    if value <= 0.0 {
        return 0;
    }
    if value >= 1.0 {
        return 255;
    }
    let mut low = 0usize;
    let mut high = 255usize;
    while high - low > 1 {
        let middle = (low + high) / 2;
        if f64::from_bits(SRGB_TO_LINEAR_BITS[middle]) <= value {
            low = middle;
        } else {
            high = middle;
        }
    }
    let low_value = f64::from_bits(SRGB_TO_LINEAR_BITS[low]);
    let high_value = f64::from_bits(SRGB_TO_LINEAR_BITS[high]);
    if high_value - value < value - low_value { high as i64 } else { low as i64 }
}

/// The sRGB channel values 0-255 of OKLab coordinates, out-of-range channels clamped.
pub fn from_oklab(lab: [f64; 3]) -> [i64; 3] {
    let l_root = lab[0] + LA * lab[1] + LB * lab[2];
    let m_root = lab[0] + MA * lab[1] + MB * lab[2];
    let s_root = lab[0] + SA * lab[1] + SB * lab[2];
    let l = l_root * l_root * l_root;
    let m = m_root * m_root * m_root;
    let s = s_root * s_root * s_root;
    [
        from_linear(R1 * l + R2 * m + R3 * s),
        from_linear(G1 * l + G2 * m + G3 * s),
        from_linear(B1 * l + B2 * m + B3 * s),
    ]
}

/// The sRGB channel values of a hue in degrees and a saturation and lightness from 0 to 100:
/// C = (1 - |2L - 1|) * S, X = C * (1 - |(H / 60) mod 2 - 1|), m = L - C / 2, each channel rounded
/// half to even after multiplying by 255.
pub fn from_hsl(hue: f64, saturation: f64, lightness: f64) -> [i64; 3] {
    let h = hue - 360.0 * (hue / 360.0).floor();
    let s = saturation.clamp(0.0, 100.0) / 100.0;
    let l = lightness.clamp(0.0, 100.0) / 100.0;
    let chroma = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let sector = h / 60.0;
    let x = chroma * (1.0 - ((sector - 2.0 * (sector / 2.0).floor()) - 1.0).abs());
    let offset = l - chroma / 2.0;
    let parts = match sector.floor() as i64 {
        0 => [chroma, x, 0.0],
        1 => [x, chroma, 0.0],
        2 => [0.0, chroma, x],
        3 => [0.0, x, chroma],
        4 => [x, 0.0, chroma],
        _ => [chroma, 0.0, x],
    };
    parts.map(|part| round_even((part + offset) * 255.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_channel_comes_back() {
        for channel in 0..=255 {
            assert_eq!(from_linear(srgb_to_linear(channel)), channel);
        }
    }

    #[test]
    fn red_in_oklab() {
        let lab = to_oklab([255, 0, 0]);
        assert_eq!(format!("{:.15}", lab[0]), "0.627955360614552");
        assert_eq!(format!("{:.15}", lab[1]), "0.224863061065974");
        assert_eq!(format!("{:.15}", lab[2]), "0.125846298530735");
        assert_eq!(from_oklab(lab), [255, 0, 0]);
    }

    #[test]
    fn hsl_forms() {
        assert_eq!(from_hsl(30.0, 100.0, 50.0), [255, 128, 0]);
        assert_eq!(from_hsl(-120.0, 50.0, 25.0), [32, 32, 96]);
        assert_eq!(from_hsl(15.0, 100.0, 60.0), [255, 102, 51]);
    }
}
